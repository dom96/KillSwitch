# Tests

The tests that I run with LLMs to see if they can edit/create/understand KillSwitch code correctly.

All tests here are executed without the AGENTS.md file present in the tests/ dir.

## Max.ks understanding

With max.ks renamed to test1.ks. Ask:

```
Read SPEC.md and tests/test1.ks then explain to me exactly what tests/test1.ks will output. Do not read any other files.
```

Interesting thing here is that LLMs are able to use any leaked info very well. Even just my chapter having the name "maximally"
gave it a clue that it returns the max of the inputs. Removing this info immediately made it much harder for it.

### GPT-5.6 Sol

It gets caught out thinking that execution is top-to-bottom. It cannot determine the "exact stdout".

### Kimi 2.7

Also thinks execution is top-to-bottom. States that execution will hang waiting on stdin.

### Opus 5

Same as the others. It seems to consider reversing the execution, but then makes other bad assumptions that make it think that execution order also doesn't work.

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

Runtime: 47s

Cost: $0.07

- "Invalid func call, need an anagram of deductively"
- Re-prompting with above fixed this, but there is also a bad value ref in there.
- Re-prompting fixed that, but then I get wrong output
- works after 4th prompt

### Fable 5

Runtime: 22.3s

Cost: $0.30

- "The response was blocked by the provider's content filter"

I tried to remove references to hack/bomb/etc and it did get further. But as it
started writing the celsius.ks file it triggered the filter again.

## Analysing fahrenheit.ks

Many models trigger their internal filters if I ask them to improve the story, or fix the code without giving them
any context that this is a script.
