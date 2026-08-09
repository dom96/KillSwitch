# Tests

The tests that I run with LLMs to see if they can edit/create KillSwitch code correctly.

## Fahrenheit.ks -> Celsius.ks

Prompt: `Read SPEC.md then please implement celsius.ks which does the opposite conversion to fahrenheit.ks.`

This TESTS.md file isn't present for the test.

### Kimi 2.7

Runtime: 8 minutes

Cost: $0.19

- Incorrect "DEDUCTIVELY" call. Not enough words after it (10 vs 11)
- Incorrect "divisely" call. Not enough words.
- Reused "scorchingly" for both chapter and story names.

### Opus 4.5

Runtime: 37.2s

Cost: $0.16

- Starts by calling "visibly" which will fail because there is nothing on the stack.
- Tries to call "deductively" using the "yducdeivelt" anagram, but not the correct
  number of words after the call (9 when it should be 11)
- Uses the same name for story and chapter.
- Asks for a value 2 lines below, when there is only 1 line below it. Also does it outside chapter.

### Sonnet 4.5

Runtime: 1m 11s

Cost: $0.14

- Calls divisibly after calling stdin.read (humanely), will fail because there is only one value on stack
- Then calls deductively, with the wrong count of words after it.
- Then calls "icily", keeps one word after which is incorrect, because 'r' isn't in the adverb.
