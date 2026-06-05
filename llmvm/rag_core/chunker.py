"""
Document chunker. Splits text into semantic chunks.
Uses tiktoken for accurate token counting.
Fallback: character-based splitting if tiktoken unavailable.
"""

import os
import re
from dataclasses import dataclass, field
from typing import List, Optional


@dataclass
class Chunk:
    text: str
    index: int
    source: str
    token_count: int = 0
    metadata: dict = field(default_factory=dict)


CODE_EXTENSIONS = frozenset({
    ".rs", ".py", ".js", ".ts", ".go", ".java", ".c", ".cpp", ".h", ".hpp",
    ".cc", ".cxx", ".kt", ".kts", ".swift", ".rb", ".php", ".lua", ".dart",
    ".r", ".m", ".sh", ".bash", ".zsh", ".ada", ".adb", ".ads", ".sql",
    ".toml", ".yaml", ".yml", ".json", ".css", ".scss", ".html", ".htm",
    ".svelte", ".vue", ".erl", ".ex", ".exs", ".hs", ".lhs", ".clj",
    ".cljs", ".zig", ".nim", ".cr", ".scala", ".ml", ".mli", ".fs", ".fsx",
})


class TextChunker:
    """
    Splits documents into overlapping chunks by token count.
    Respects paragraph and sentence boundaries when possible.
    For code files, splits at definition boundaries (fn/class/struct/impl/trait/etc).
    """

    def __init__(self, chunk_size: int = 512, chunk_overlap: int = 64):
        self.chunk_size = chunk_size
        self.chunk_overlap = chunk_overlap
        self._tokenizer = self._init_tokenizer()

    @staticmethod
    def is_code_source(source: str) -> bool:
        _, ext = os.path.splitext(source.lower())
        return ext in CODE_EXTENSIONS

    @staticmethod
    def is_definition_boundary(line: str) -> bool:
        t = line.strip()
        return (
            t.startswith("fn ") or t.startswith("pub fn") or t.startswith("pub(crate) fn")
            or t.startswith("pub(super) fn") or t.startswith("pub unsafe fn")
            or t.startswith("unsafe fn") or t.startswith("def ")
            or t.startswith("async def ")
            or t.startswith("class ") or t.startswith("public class ")
            or t.startswith("private class ") or t.startswith("protected class ")
            or t.startswith("struct ") or t.startswith("pub struct ")
            or t.startswith("impl ") or t.startswith("pub impl ") or t.startswith("trait ")
            or t.startswith("pub trait ") or t.startswith("enum ") or t.startswith("pub enum ")
            or t.startswith("interface ") or t.startswith("type ")
            or t.startswith("func ") or t.startswith("function ")
            or t.startswith("sub ") or t.startswith("public function ")
            or t.startswith("private function ") or t.startswith("public static function ")
            or t.startswith("async fn") or t.startswith("export function")
            or t.startswith("export async function") or t.startswith("defn ")
            or t.startswith("CREATE ") or t.startswith("ALTER ") or t.startswith("DROP ")
            or t.startswith("SELECT ") or t.startswith("INSERT ") or t.startswith("UPDATE ")
            or t.startswith("DELETE ") or t.startswith("pub type") or t.startswith("pub enum")
            or t.startswith("macro_rules!")
            or (t.startswith("#[") and "]" in t)
        )

    @staticmethod
    def split_code_segments(text: str) -> list[str]:
        lines = text.splitlines()
        segments = []
        start_line = 0
        for i, line in enumerate(lines):
            if i > 0 and TextChunker.is_definition_boundary(line):
                segment = "\n".join(lines[start_line:i])
                if segment.strip():
                    segments.append(segment)
                start_line = i
        remaining = "\n".join(lines[start_line:])
        if remaining.strip():
            segments.append(remaining)
        if not segments:
            segments = [text]
        return segments

    def chunk_code(self, text: str, source: str) -> list[Chunk]:
        segments = self.split_code_segments(text)
        chunks = []
        current = ""
        current_tokens = 0
        half_size = self.chunk_size * 2

        for segment in segments:
            seg_tokens = self._count_tokens(segment)
            if current_tokens + seg_tokens <= half_size:
                current += ("\n\n" if current else "") + segment
                current_tokens += seg_tokens
                continue
            if current:
                chunks.append(Chunk(
                    text=current,
                    index=len(chunks),
                    source=source,
                    token_count=current_tokens,
                ))
            current = segment
            current_tokens = seg_tokens

        if current:
            chunks.append(Chunk(
                text=current,
                index=len(chunks),
                source=source,
                token_count=current_tokens,
            ))

        return chunks

    def _init_tokenizer(self):
        """Try tiktoken, fallback to None (char-based counting)."""
        try:
            import tiktoken
            return tiktoken.get_encoding("cl100k_base")  # GPT-4 tokenizer
        except ImportError:
            return None

    def _count_tokens(self, text: str) -> int:
        if self._tokenizer:
            return len(self._tokenizer.encode(text))
        # Fallback: ~1 token per 4 chars (rough estimate)
        return len(text) // 4

    def _split_by_paragraphs(self, text: str) -> List[str]:
        """Split text by paragraph boundaries."""
        # Normalize line endings
        text = text.replace('\r\n', '\n').replace('\r', '\n')
        # Split on double newlines (paragraph breaks)
        paragraphs = re.split(r'\n\s*\n', text)
        # Filter empty and strip
        return [p.strip() for p in paragraphs if p.strip()]

    def _split_by_sentences(self, text: str) -> List[str]:
        """Split paragraph into sentences."""
        # Rough sentence split (handles common abbreviations poorly, but good enough)
        sentences = re.split(r'(?<=[.!?])\s+', text)
        return [s for s in sentences if s.strip()]

    def chunk(self, text: str, source: str = "unknown") -> List[Chunk]:
        """
        Split text into overlapping chunks respecting semantic boundaries.
        For code files, splits at definition boundaries (fn/class/struct/impl/trait/etc).
        For prose, priority: paragraphs > sentences > hard token split.
        """
        if not text.strip():
            return []

        if self.is_code_source(source):
            return self.chunk_code(text, source)

        chunks = []
        paragraphs = self._split_by_paragraphs(text)

        current_text = ""
        current_tokens = 0

        for para in paragraphs:
            para_tokens = self._count_tokens(para)

            # Paragraph fits within chunk
            if current_tokens + para_tokens <= self.chunk_size:
                current_text += ("\n\n" if current_text else "") + para
                current_tokens += para_tokens
                continue

            # Paragraph too big - split by sentences
            if para_tokens > self.chunk_size:
                # Flush current buffer
                if current_text:
                    chunks.append(Chunk(
                        text=current_text,
                        index=len(chunks),
                        source=source,
                        token_count=current_tokens
                    ))
                    current_text = ""
                    current_tokens = 0

                # Split paragraph by sentences
                sentences = self._split_by_sentences(para)
                sent_buffer = ""
                sent_tokens = 0

                for sentence in sentences:
                    s_tokens = self._count_tokens(sentence)
                    if sent_tokens + s_tokens <= self.chunk_size:
                        sent_buffer += (" " if sent_buffer else "") + sentence
                        sent_tokens += s_tokens
                    else:
                        if sent_buffer:
                            chunks.append(Chunk(
                                text=sent_buffer,
                                index=len(chunks),
                                source=source,
                                token_count=sent_tokens
                            ))
                            # Start overlap
                            overlap_text = sent_buffer[-self.chunk_overlap:] if sent_tokens > self.chunk_overlap else ""
                            overlap_tokens = self._count_tokens(overlap_text)
                            sent_buffer = overlap_text + (" " if overlap_text else "") + sentence
                            sent_tokens = overlap_tokens + s_tokens
                        else:
                            # Single sentence exceeds chunk - hard split
                            words = sentence.split()
                            word_buffer = []
                            word_tokens = 0
                            for word in words:
                                w_tok = self._count_tokens(word)
                                if word_tokens + w_tok > self.chunk_size and word_buffer:
                                    hard_chunk = " ".join(word_buffer)
                                    chunks.append(Chunk(
                                        text=hard_chunk,
                                        index=len(chunks),
                                        source=source,
                                        token_count=word_tokens
                                    ))
                                    word_buffer = word_buffer[-self.chunk_overlap // 4:]  # word-level overlap
                                    word_tokens = sum(self._count_tokens(w) for w in word_buffer)
                                word_buffer.append(word)
                                word_tokens += w_tok
                            if word_buffer:
                                chunks.append(Chunk(
                                    text=" ".join(word_buffer),
                                    index=len(chunks),
                                    source=source,
                                    token_count=word_tokens
                                ))
                            sent_buffer = ""
                            sent_tokens = 0

                if sent_buffer:
                    chunks.append(Chunk(
                        text=sent_buffer,
                        index=len(chunks),
                        source=source,
                        token_count=sent_tokens
                    ))
                continue

            # Adding this paragraph would exceed limit - create overlap
            if current_text:
                overlap_start = max(0, len(current_text) - self.chunk_overlap)
                current_text = current_text[overlap_start:] + ("\n\n" if current_text[overlap_start:] else "") + para
                current_tokens = self._count_tokens(current_text)

                chunks.append(Chunk(
                    text=current_text[:self._find_boundary(current_text, self.chunk_size)],
                    index=len(chunks),
                    source=source,
                    token_count=self.chunk_size
                ))
                current_text = current_text[self._find_boundary(current_text, self.chunk_size):].lstrip()
                current_tokens = self._count_tokens(current_text)

        # Flush remaining
        if current_text.strip():
            chunks.append(Chunk(
                text=current_text.strip(),
                index=len(chunks),
                source=source,
                token_count=current_tokens
            ))

        return chunks

    def _find_boundary(self, text: str, max_tokens: int) -> int:
        """Find a safe split point near max_tokens."""
        if self._tokenizer:
            tokens = self._tokenizer.encode(text)
            if len(tokens) <= max_tokens:
                return len(text)
            # Decode back to find byte position
            truncated = self._tokenizer.decode(tokens[:max_tokens])
            return len(truncated)
        return min(len(text), max_tokens * 4)


def chunk_file(filepath: str, chunk_size: int = 512, chunk_overlap: int = 64) -> List[Chunk]:
    """Read a file and chunk it."""
    with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
        text = f.read()
    chunker = TextChunker(chunk_size, chunk_overlap)
    return chunker.chunk(text, source=filepath)
