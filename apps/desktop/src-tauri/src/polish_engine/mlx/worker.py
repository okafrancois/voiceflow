import argparse
from contextlib import redirect_stdout
import gc
from importlib.metadata import version
import json
import os
from pathlib import Path
import sys
import threading
import time
import traceback


MODELS = {"mlx-qwen3-0.6b", "mlx-qwen3-1.7b"}
MAX_REQUEST_BYTES = 1024 * 1024


class WorkerState:
    def __init__(self, models_dir, loader, generate, clear_cache):
        self.models_dir = Path(models_dir).resolve()
        self.loader = loader
        self.generate = generate
        self.clear_cache = clear_cache
        self.model = None
        self.tokenizer = None
        self.model_name = None

    def load(self, name):
        if name not in MODELS:
            raise ValueError("Unknown managed MLX polish model")
        path = (self.models_dir / name).resolve()
        if path.parent != self.models_dir or not (path / "config.json").is_file():
            raise ValueError("Polish model must be in the application model directory")
        if self.model_name == name and self.model is not None:
            return 0
        if self.model is not None:
            self.model = None
            self.tokenizer = None
            self.model_name = None
            gc.collect()
            self.clear_cache()
        started = time.monotonic()
        self.model, self.tokenizer = self.loader(path)
        self.model_name = name
        return int((time.monotonic() - started) * 1000)


def handle_request(request, state):
    if not isinstance(request, dict) or type(request.get("id")) is not int:
        raise ValueError("A numeric request ID is required")
    action = request.get("action")
    if action not in {"load", "polish"}:
        raise ValueError("Unsupported polish action")
    if action == "polish":
        for field in ["text", "system_prompt"]:
            if not isinstance(request.get(field), str):
                raise ValueError("Polish text and system prompt must be strings")
        if type(request.get("max_tokens")) is not int or not 1 <= request["max_tokens"] <= 4096:
            raise ValueError("Invalid polish output token limit")
    model_load_ms = state.load(request.get("model"))
    response = {"id": request["id"], "ok": True, "model_load_ms": model_load_ms}
    if action == "load":
        return response
    prompt = state.tokenizer.apply_chat_template(
        [
            {"role": "system", "content": request["system_prompt"]},
            {"role": "user", "content": request["text"]},
        ],
        tokenize=False,
        add_generation_prompt=True,
        enable_thinking=False,
    )
    started = time.monotonic()
    chunks = []
    finish_reason = None
    for chunk in state.generate(
        state.model, state.tokenizer, prompt=prompt, max_tokens=request["max_tokens"]
    ):
        chunks.append(chunk.text)
        finish_reason = chunk.finish_reason
    if finish_reason != "stop":
        raise ValueError("Polish output exceeded its token limit or ended incompletely")
    response.update(text="".join(chunks).strip(), inference_ms=int((time.monotonic() - started) * 1000))
    return response


def serve(input_stream, output_stream, diagnostics, state):
    while True:
        line = input_stream.readline(MAX_REQUEST_BYTES + 1)
        if not line:
            return
        request = None
        try:
            if len(line.encode("utf-8")) > MAX_REQUEST_BYTES:
                raise ValueError("Polish request exceeds the protocol limit")
            request = json.loads(line)
            with redirect_stdout(diagnostics):
                response = handle_request(request, state)
        except Exception as error:
            traceback.print_exc(file=diagnostics)
            response = {
                "id": request.get("id") if isinstance(request, dict) else None,
                "ok": False,
                "error": str(error),
            }
        output_stream.write(json.dumps(response, ensure_ascii=False) + "\n")
        output_stream.flush()


def watch_parent(parent_pid):
    while True:
        time.sleep(1)
        if os.getppid() != parent_pid:
            os._exit(0)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--models-dir", required=True)
    parser.add_argument("--parent-pid", type=int, required=True)
    args = parser.parse_args()
    threading.Thread(target=watch_parent, args=(args.parent_pid,), daemon=True).start()
    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["TRANSFORMERS_OFFLINE"] = "1"
    os.environ["HF_HUB_DISABLE_TELEMETRY"] = "1"
    with redirect_stdout(sys.stderr):
        import mlx.core as mx
        from mlx_lm import load, stream_generate
        from mlx_lm.sample_utils import make_sampler

        def loader(path):
            return load(str(path), tokenizer_config={"trust_remote_code": False})

        def generate(model, tokenizer, **options):
            return stream_generate(model, tokenizer, sampler=make_sampler(temp=0), **options)

        state = WorkerState(args.models_dir, loader, generate, mx.clear_cache)
    sys.stdout.write(json.dumps({
        "protocol": 1, "kind": "ready", "mlx_lm_version": version("mlx-lm"), "mlx_version": version("mlx")
    }) + "\n")
    sys.stdout.flush()
    serve(sys.stdin, sys.stdout, sys.stderr, state)


if __name__ == "__main__":
    main()
