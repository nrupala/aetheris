"""Offline unit tests for the Ollama chat client (no network)."""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, ROOT)

from src import generate  # noqa: E402


class _Resp:
    def __init__(self, payload):
        self._payload = payload

    def raise_for_status(self):
        pass

    def json(self):
        return self._payload


def test_ollama_chat_returns_message_content(monkeypatch):
    captured = {}

    def fake_post(url, json=None, timeout=None):
        captured["url"] = url
        captured["json"] = json
        return _Resp({"message": {"role": "assistant", "content": "hello [1]"}})

    monkeypatch.setattr(generate.httpx, "post", fake_post)
    out = generate.ollama_chat([{"role": "user", "content": "hi"}], model="m", url="http://x")
    assert out == "hello [1]"
    assert captured["url"].endswith("/api/chat")
    assert captured["json"]["model"] == "m"
    assert captured["json"]["stream"] is False


def test_ollama_chat_raises_without_message(monkeypatch):
    def fake_post(url, json=None, timeout=None):
        return _Resp({"unexpected": True})

    monkeypatch.setattr(generate.httpx, "post", fake_post)
    raised = False
    try:
        generate.ollama_chat([{"role": "user", "content": "hi"}])
    except RuntimeError:
        raised = True
    assert raised
