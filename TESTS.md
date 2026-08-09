# Tests

The tests that I run with LLMs to see if they can edit/create KillSwitch code correctly.

## Fahrenheit.ks -> Celsius.ks

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
