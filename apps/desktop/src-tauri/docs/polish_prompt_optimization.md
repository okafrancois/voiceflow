# Polish Prompt Optimization for Small Models (<3B)

## Problem

With <3B parameter models (Qwen 0.8B-2B, LFM 1.2B-2.6B), the original prompts were:
1. **Too verbose**: Long instructions consume tokens and confuse small models
2. **Single-language examples**: Misled models into translating input instead of preserving its language
3. **Complex formatting**: Agent template had too many markdown rules

Example failure:
- Input: A structured product-documentation request in another language
- Output: An English translation instead of a polished version in the source language

## Optimization Strategy

### 1. Simplify Language
- **Before**: "You are a text polishing assistant. Your job is MINIMAL editing."
- **After**: "Polish text minimally. Keep the SAME language as input."

### 2. Reduce Token Count
- Removed verbose explanations
- Shortened rule descriptions
- Kept only essential instructions

### 3. Add Multilingual Examples
Every template now includes examples that demonstrate language preservation:
```
- "Um, I think this is good" → "I think this is good"
- A non-English filler example → the same language without fillers
```

### 4. Emphasize Language Preservation
Moved "SAME LANGUAGE" to the first rule in every prompt:
```
RULES:
1. SAME LANGUAGE: preserve the input language
```

## Changes by Template

### Default Polish Prompt
- **Token reduction**: ~180 → ~90 tokens
- **Key change**: Added multilingual examples and simplified rules
- **Focus**: Minimal editing, language preservation

### Filler Template
- **Token reduction**: ~150 → ~80 tokens
- **Key change**: Added non-English filler examples
- **Focus**: Remove fillers only, no rewriting

### Formal Template
- **Token reduction**: ~120 → ~90 tokens
- **Key change**: Added a non-English formal conversion example
- **Focus**: Style conversion while preserving language

### Concise Template
- **Token reduction**: ~130 → ~85 tokens
- **Key change**: Added a non-English conciseness example
- **Focus**: Shorten without losing meaning

### Agent Template (Most Critical)
- **Token reduction**: ~250 → ~110 tokens (56% reduction!)
- **Key changes**:
  - Removed complex formatting guidelines
  - Added a non-English markdown example
  - Simplified to basic structure (## headers, - lists)
  - Removed "Requirements" section complexity
- **Focus**: Simple markdown formatting, language preservation

## Performance Benefits

1. **Faster inference**: Fewer prompt tokens = faster generation
2. **Better accuracy**: Simpler instructions = better following
3. **Language preservation**: Bilingual examples prevent translation
4. **Lower memory**: Shorter prompts fit better in small model context

## Testing

All templates tested with:
- English input → English output ✓
- Non-English input → output in the same language ✓
- Mixed content handling ✓
- No-change scenarios ✓

## Recommendations

For <3B models:
1. Keep prompts under 100 tokens when possible
2. Always include examples in target languages
3. Use simple, direct language
4. Avoid complex multi-step instructions
5. Emphasize critical rules (like language preservation) multiple times
