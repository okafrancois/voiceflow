import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
import wave


WORKER_PATH = Path(__file__).parents[1] / "src/stt_engine/mlx/worker.py"
SPEC = importlib.util.spec_from_file_location("voiceflow_mlx_worker", WORKER_PATH)
worker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(worker)


class Model:
    def __init__(self, calls):
        self.calls = calls

    def generate(self, audio, **options):
        self.calls.append((audio, options))
        print("backend diagnostic")
        return type("Result", (), {"text": "Bonjour, voici la fin de ma phrase."})()


class WorkerTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.models = self.root / "models"
        self.audio = self.root / "audio"
        self.audio.mkdir()
        self.loads = []
        self.calls = []
        self.clears = []
        for name in worker.MODEL_FAMILIES:
            path = self.models / name
            path.mkdir(parents=True)
            (path / "config.json").write_text("{}")
        self.state = worker.WorkerState(
            self.models,
            self.audio,
            lambda path: self.load(path),
            lambda: self.clears.append(True),
        )
        self.wav = self.audio / "recording.wav"
        with wave.open(str(self.wav), "wb") as output:
            output.setnchannels(1)
            output.setsampwidth(2)
            output.setframerate(16000)
            output.writeframes(b"\x01\x00" * 16003)

    def load(self, path):
        self.loads.append(path)
        return Model(self.calls)

    def request(self, **fields):
        return worker.handle_request({"id": 1, **fields}, self.state)

    def test_transcription_preserves_language_prompt_and_protocol_stdout(self):
        protocol = io.StringIO()
        diagnostics = io.StringIO()
        request = {
            "id": 7,
            "action": "transcribe",
            "model": "mlx-whisper-turbo",
            "audio": str(self.wav),
            "language": "fr-FR",
            "initial_prompt": "Voice Flow, MLX",
        }
        worker.serve(io.StringIO(json.dumps(request) + "\n"), protocol, diagnostics, self.state)
        response = json.loads(protocol.getvalue())
        self.assertEqual(response["id"], 7)
        self.assertTrue(response["ok"])
        self.assertEqual(response["text"], "Bonjour, voici la fin de ma phrase.")
        self.assertIn("backend diagnostic", diagnostics.getvalue())
        self.assertEqual(self.calls[0][1]["language"], "fr")
        self.assertEqual(self.calls[0][1]["initial_prompt"], "Voice Flow, MLX")

    def test_model_is_reused_and_released_on_switch(self):
        self.request(action="load", model="mlx-whisper-turbo")
        self.request(action="load", model="mlx-whisper-turbo")
        self.assertEqual(len(self.loads), 1)
        self.request(action="load", model="mlx-parakeet-v3")
        self.assertEqual(len(self.loads), 2)
        self.assertEqual(len(self.clears), 1)

    def test_qwen_uses_language_names_and_its_native_prompt(self):
        self.request(action="transcribe", model="mlx-qwen3-asr-0.6b", audio=str(self.wav), language="fr", initial_prompt="Voice Flow")
        self.assertEqual(self.calls[0][1]["language"], "French")
        self.assertEqual(self.calls[0][1]["system_prompt"], "Voice Flow")
        self.assertNotIn("initial_prompt", self.calls[0][1])

    def test_unknown_models_and_paths_outside_private_audio_directory_are_rejected(self):
        with self.assertRaises(ValueError):
            self.request(action="load", model="../../outside")
        with self.assertRaises(ValueError):
            self.request(action="transcribe", model="mlx-whisper-turbo", audio=str(self.root / "outside.wav"))

    def test_non_16khz_mono_pcm_audio_is_rejected(self):
        with wave.open(str(self.wav), "wb") as output:
            output.setnchannels(2)
            output.setsampwidth(2)
            output.setframerate(48000)
            output.writeframes(b"\x00\x00" * 32)
        with self.assertRaises(ValueError):
            self.request(action="transcribe", model="mlx-whisper-turbo", audio=str(self.wav))

    def test_invalid_json_reports_failure_and_next_request_still_works(self):
        protocol = io.StringIO()
        requests = "not JSON\n" + json.dumps({"id": 2, "action": "load", "model": "mlx-whisper-turbo"}) + "\n"
        worker.serve(io.StringIO(requests), protocol, io.StringIO(), self.state)
        responses = [json.loads(line) for line in protocol.getvalue().splitlines()]
        self.assertFalse(responses[0]["ok"])
        self.assertTrue(responses[1]["ok"])
        self.assertEqual(responses[1]["id"], 2)


if __name__ == "__main__":
    unittest.main()
