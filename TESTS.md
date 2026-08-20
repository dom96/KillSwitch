# Tests

The tests that I run with LLMs to see if they can edit/create KillSwitch code correctly.

## Fahrenheit.ks -> Celsius.ks

Prompt: `Read SPEC.md then please implement celsius.ks which does the opposite conversion to fahrenheit.ks.`

Using opencode permissions, we only allow the reading of SPEC.md, src/AGENTS.md and tests/fahrenheit.ks.

### Kimi 2.7

Runtime: 10 minutes 41s

Cost: $0.16

Interesting notes:

- Uses Python to generate anagrams and count letters
- Also uses Python to verify it got the anagrams correct
- Runs successfully, no eval errors

Mistakes:

- Wrong formula. (F * 5/9) - 32
- Prompting again with example failing result leads to it making a different mistake: mentioning value, but not referencing it.
- Works after 3rd prompt.

### Opus 4.5

Runtime: 2m 45s

Cost: $0.40

Interesting notes:

- Writes out "Line 33: ...\n Line 34: ...\n" to help itself count lines.

Mistakes:

- calls `llychilgin` without the right number of words after it
- calls `YLEVITCUDED` without the right number of words after it
- after prompting about above, it failed to count words correctly again, put 12 words instead of 11
- it's royally getting confused about the "AND", it thinks that it doesn't count as part of the word count, but it does
- even after many prompts to get it to figure out the word count, it finally doesn't end up with an eval error, but it does not calculate correctly

### Sonnet 4.5

Runtime: 1m 11s

Cost: $0.14

- Calls divisibly after calling stdin.read (humanely), will fail because there is only one value on stack
- Then calls deductively, with the wrong count of words after it.
- Then calls "icily", keeps one word after which is incorrect, because 'r' isn't in the adverb.

### GPT-5.6 Terra

Runtime: 35.8s

Cost: $0.07

- No runtime errors, but it is pushing 0.55555555556 on the stack first. Order of operations wrong.

### Fable 5

Runtime: 1m 34s

Cost: $0.51

- "The response was blocked by the provider's content filter"
