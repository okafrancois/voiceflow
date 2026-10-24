import argparse
from contextlib import redirect_stdout
import gc
import json
import os
from pathlib import Path
import sys
import threading
import time
import traceback
import wave


PROTOCOL_VERSION = 1
MAX_REQUEST_BYTES = 1024 * 1024
MODEL_FAMILIES = {
    "mlx-whisper-turbo": "whisper",
    "mlx-parakeet-v3": "parakeet",
    "mlx-qwen3-asr-0.6b": "qwen3_asr",
}
LANGUAGE_NAMES = {
    "ar": "Arabic", "cs": "Czech", "da": "Danish", "de": "German",
    "el": "Greek", "en": "English", "es": "Spanish", "fa": "Persian",
    "fi": "Finnish", "fr": "French", "hi": "Hindi", "hu": "Hungarian",
    "id": "Indonesian", "it": "Italian", "ja": "Japanese", "ko": "Korean",
    "mk": "Macedonian", "ms": "Malay", "nl": "Dutch", "pl": "Polish",
    "pt": "Portuguese", "ro": "Romanian", "ru": "Russian", "sv": "Swedish",
    "th": "Thai", "tr": "Turkish", "uk": "Ukrainian", "vi": "Vietnamese",
    "yue": "Cantonese", "zh": "Chinese",
}


class WorkerState:
    def __init__(self, models_dir, audio_dir, loader, clear_cache):
        self.models_dir = Path(models_dir).resolve()
        self.audio_dir = Path(audio_dir).resolve()
        self.loader = loader
        self.clear_cache = clear_cache
        self.model = None
        self.model_name = None

    def load(self, name):
        if name not in MODEL_FAMILIES:
            raise ValueError("Unknown managed MLX model")
        model_path = (self.models_dir / name).resolve()
        if model_path.parent != self.models_dir or not (model_path / "config.json").is_file():
            raise ValueError("MLX model is not installed in the application model directory")
        if self.model_name == name and self.model is not None:
            return 0
        if self.model is not None:
            self.model = None
            self.model_name = None
            gc.collect()
            self.clear_cache()
        started = time.monotonic()
        self.model = self.loader(model_path)
        self.model_name = name
        return int((time.monotonic() - started) * 1000)

    def audio_path(self, value):
        if not isinstance(value, str):
            raise ValueError("A private WAV recording is required")
        path = Path(value).resolve()
        if path.parent != self.audio_dir or path.suffix != ".wav":
            raise ValueError("Audio must be in the application's private audio directory")
        with wave.open(str(path), "rb") as recording:
            if (recording.getnchannels(), recording.getsampwidth(), recording.getframerate(), recording.getcomptype()) != (1, 2, 16000, "NONE"):
                raise ValueError("MLX requires 16 kHz mono PCM16 WAV audio")
            if recording.getnframes() == 0:
                raise ValueError("The recording is empty")
        return path


def handle_request(request, state):
    if not isinstance(request, dict) or type(request.get("id")) is not int:
        raise ValueError("A request ID is required")
    action = request.get("action")
    if action not in {"load", "transcribe"}:
        raise ValueError("Unsupported MLX worker action")
    model_name = request.get("model")
    audio = state.audio_path(request.get("audio")) if action == "transcribe" else None
    model_load_ms = state.load(model_name)
    response = {"id": request["id"], "ok": True, "model_load_ms": model_load_ms}
    if action == "load":
        return response
    language = request.get("language")
    if language:
        if not isinstance(language, str):
            raise ValueError("Language must be a string")
        language = language.replace("_", "-").split("-")[0].lower()
    if language == "auto":
        language = None
    prompt = request.get("initial_prompt")
    if prompt is not None and not isinstance(prompt, str):
        raise ValueError("Initial prompt must be a string")
    options = {"verbose": False}
    family = MODEL_FAMILIES[model_name]
    if family == "whisper":
        if language:
            options["language"] = language
        if prompt:
            options["initial_prompt"] = prompt
    elif family == "qwen3_asr":
        if language:
            options["language"] = LANGUAGE_NAMES.get(language, language)
        if prompt:
            options["system_prompt"] = prompt
    started = time.monotonic()
    result = state.model.generate(str(audio), **options)
    if not isinstance(getattr(result, "text", None), str):
        raise ValueError("MLX returned an invalid transcription result")
    response.update(text=result.text.strip(), inference_ms=int((time.monotonic() - started) * 1000))
    return response


def serve(input_stream, output_stream, diagnostics, state):
    while True:
        line = input_stream.readline(MAX_REQUEST_BYTES + 1)
        if not line:
            return
        request = None
        try:
            if len(line) > MAX_REQUEST_BYTES:
                raise ValueError("MLX request exceeds the protocol size limit")
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
        if len(line) > MAX_REQUEST_BYTES:
            return


def monitor_parent(parent_pid):
    while os.getppid() == parent_pid:
        time.sleep(1)
    os._exit(0)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--models-dir", required=True)
    parser.add_argument("--audio-dir", required=True)
    parser.add_argument("--parent-pid", required=True, type=int)
    arguments = parser.parse_args()
    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["TRANSFORMERS_OFFLINE"] = "1"
    os.environ["HF_HUB_DISABLE_TELEMETRY"] = "1"
    threading.Thread(target=monitor_parent, args=(arguments.parent_pid,), daemon=True).start()
    with redirect_stdout(sys.stderr):
        from importlib.metadata import version
        import mlx.core as mx
        from mlx_audio.stt import load

        installed_version = version("mlx-audio")
        if installed_version != "0.6.1":
            raise RuntimeError("The managed MLX Audio runtime version is incompatible")
        state = WorkerState(arguments.models_dir, arguments.audio_dir, load, mx.clear_cache)
    print(json.dumps({"protocol": PROTOCOL_VERSION, "kind": "ready", "mlx_audio_version": installed_version}), flush=True)
    serve(sys.stdin, sys.stdout, sys.stderr, state)


if __name__ == "__main__":
    main()
