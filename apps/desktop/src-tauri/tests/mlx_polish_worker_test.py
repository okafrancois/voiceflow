import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest


WORKER_PATH = Path(__file__).parents[1] / "src/polish_engine/mlx/worker.py"
SPEC = importlib.util.spec_from_file_location("voiceflow_mlx_polish_worker", WORKER_PATH)
worker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(worker)


class Tokenizer:
    def apply_chat_template(self, messages, **options):
        self.messages = messages
        self.options = options
        return "formatted prompt"


class WorkerTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.models = Path(self.directory.name)
        self.loads = []
        self.clears = []
        self.calls = []
        self.tokenizer = Tokenizer()
        for name in worker.MODELS:
            directory = self.models / name
            directory.mkdir()
            (directory / "config.json").write_text("{}")
        self.state = worker.WorkerState(
            self.models, self.load, self.generate, lambda: self.clears.append(True)
        )

    def load(self, path):
        self.loads.append(path)
        return object(), self.tokenizer

    def generate(self, model, tokenizer, **options):
        self.calls.append(options)
        print("library diagnostic")
        yield type("Chunk", (), {"text": "Bonjour, ", "finish_reason": None})()
        yield type("Chunk", (), {"text": "je suis allé à Paris.", "finish_reason": "stop"})()

    def request(self, **changes):
        return dict(id=1, action="polish", model="mlx-qwen3-0.6b", text="bonjour je suis aller a paris", system_prompt="French only", max_tokens=128, **changes)

    def test_preserves_messages_and_disables_thinking(self):
        result = worker.handle_request(self.request(), self.state)
        self.assertEqual(result["text"], "Bonjour, je suis allé à Paris.")
        self.assertEqual(self.tokenizer.messages, [
            {"role": "system", "content": "French only"},
            {"role": "user", "content": "bonjour je suis aller a paris"},
        ])
        self.assertFalse(self.tokenizer.options["enable_thinking"])
        self.assertEqual(self.calls[0]["max_tokens"], 128)

    def test_switching_releases_previous_model(self):
        worker.handle_request({"id": 1, "action": "load", "model": "mlx-qwen3-0.6b"}, self.state)
        worker.handle_request({"id": 2, "action": "load", "model": "mlx-qwen3-0.6b"}, self.state)
        self.assertEqual(len(self.loads), 1)
        worker.handle_request({"id": 3, "action": "load", "model": "mlx-qwen3-1.7b"}, self.state)
        self.assertEqual(len(self.loads), 2)
        self.assertEqual(self.clears, [True])

    def test_rejects_unknown_models_and_symlink_escape(self):
        for name in ["../escape", "arbitrary.gguf"]:
            with self.assertRaises(ValueError):
                worker.handle_request({"id": 1, "action": "load", "model": name}, self.state)
        with tempfile.TemporaryDirectory() as outside:
            directory = self.models / "mlx-qwen3-0.6b"
            (directory / "config.json").unlink()
            directory.rmdir()
            directory.symlink_to(outside, target_is_directory=True)
            with self.assertRaises(ValueError):
                worker.handle_request(self.request(), self.state)

    def test_length_limited_generation_is_never_accepted_as_complete(self):
        def truncated(*args, **kwargs):
            yield type("Chunk", (), {"text": "incomplete", "finish_reason": "length"})()
        self.state.generate = truncated
        with self.assertRaises(ValueError):
            worker.handle_request(self.request(), self.state)

    def test_protocol_stdout_remains_json_only_and_recovers_after_error(self):
        source = io.StringIO("not json\n" + json.dumps(self.request()) + "\n")
        output = io.StringIO()
        diagnostics = io.StringIO()
        worker.serve(source, output, diagnostics, self.state)
        responses = [json.loads(line) for line in output.getvalue().splitlines()]
        self.assertFalse(responses[0]["ok"])
        self.assertTrue(responses[1]["ok"])
        self.assertIn("library diagnostic", diagnostics.getvalue())

    def test_rejects_invalid_request_shape_and_limits(self):
        for changes in [{"id": True}, {"action": "http"}, {"max_tokens": 0}, {"text": 10}]:
            request = self.request()
            request.update(changes)
            with self.assertRaises(ValueError):
                worker.handle_request(request, self.state)


if __name__ == "__main__":
    unittest.main()
