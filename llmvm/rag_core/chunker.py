"""
Document chunker. Splits text into semantic chunks.
Uses tiktoken for accurate token counting.
Fallback: character-based splitting if tiktoken unavailable.
"""

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


class TextChunker:
    """
    Splits documents into overlapping chunks by token count.
    Respects paragraph and sentence boundaries when possible.
    """

    def __init__(self, chunk_size: int = 512, chunk_overlap: int = 64):
        self.chunk_size = chunk_size
        self.chunk_overlap = chunk_overlap
        self._tokenizer = self._init_tokenizer()

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
        Priority: paragraphs > sentences > hard token split.
        """
        if not text.strip():
            return []

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
