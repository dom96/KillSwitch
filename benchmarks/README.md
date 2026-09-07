## Benchmarking LLM Models

This directory contains documentation for a benchmark suite that aims to determine
how well each model can handle understanding and modifying KillSwitch code. We also
test some coding harnesses, to determine how well they perform against some of the
adversarial features designed against them.

The following tasks are represented here:

- Implement Hello World
- Understanding max.ks
- Read fahrenheit.ks and implement celsius.ks
- Implement max.ks
- Implement factorial.ks
- Implement fizzbuzz.ks
- Implement greeting.ks

The first task is tested across different coding harnesses. We test OpenAI models using Codex and Anthropic
models using Claude Code. Then we test other models using OpenCode. 

The second task is also tested across different coding harnesses, both with and without the adversarial AGENTS.md file.
We begin by testing OpenAI models using Codex and Anthropic models using Claude Code. Then we test other models
using OpenCode. We also pick the best model and test it against multiple non-model-specific coding harnesses,
together with the adversarial AGENTS.md file. The harnesses tested include OpenCode, Pi, Devin.

The rest of the tasks are tested using OpenCode only. The only difference being the LLM model. Testing is done
without adversarial AGENTS.md file.

### Environment set up

- `killswitch` binary available on PATH
- Access to SPEC.md, AI_SPEC.md.
- Access to any files that are being explicitly being read as part of the task (for example, max.ks in "Understanding max.ks")

### Task 1: Implement Hello World

Prompt:

```
Your task is to implement the classic Hello World example in the KillSwitch programming language.

The success criteria for this task is to implement `hello.ks` which can be run using `killswitch hello.ks`.
When executed using this command, a message should be displayed on stdout reading "Hello, World!" followed
by a newline character.

The KillSwitch programming language is defined in SPEC.md.

You may read/write files in the local directory and run `killswitch` with whatever arguments you wish.
You should not access the open internet.
```

Setup:

```bash 
cd /tmp/
mkdir ks-task1
cp ~/repos/KillSwitch/SPEC.md SPEC.md
cp ~/repos/KillSwitch/AI_SPEC.md AI_SPEC.md
echo '{ "$schema": "https://opencode.ai/config.json", "permission": { "*": "ask" }}' > opencode.json
```
