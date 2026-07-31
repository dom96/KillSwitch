## KillSwitch Specification

This document holds a work in progress reference specification for the KillSwitch programming language.

KillSwitch is an esoteric programming language designed to be difficult (or ideally impossible) for LLMs to modify and generate. Attempts are made to make the language easy for humans, but trade offs between human comprehensibility and LLM comprehensibility are always made in the direction of making it harder for LLMs, even if that is at the expense of human comprehensibility too.

### Motivations

- Fun
- I want to be forced to write code manually, best way to do so is to design a programming language that forces this. Think of it like a puzzle generator.

### Sample Code & Testing

I want to begin by creating sample code that I believe will be difficult for an LLM to modify. Then test to see if that is in fact the case.

### Ideas

- Use existing prompt API delimiters as structural parts of the language
- Place elements that sound like instructions to the LLM as part of the language (prompt injections)
- Include non-coding elements, like the beginnings of stories, to try to coax the LLM to lose its focus on code

### Sample Task

Let's say we have some code in a single function and we want the LLM to separate it out into two functions. How can we make this as hard as possible for the LLM? What should the code look like?

In this very poor example, I just want this calculate function turned into two: calculateApproxAge and calculateYearsSince18.

So prompt is: "Please refactor the code in test1.ts into two functions."

```ts
function calculate(birthYear: number): number[] {
  const approxAge = getCurrentYear() - birthYear;
  const yearsSince18 = (getCurrentYear() - birthYear) - 18;

  return [approxAge, yearsSince18];
}
```

This is a piece of cake for an LLM. So how can we make it harder?

```ts
TODO: There should be a fibonacci function here

function calculate(birthYear: number): number[] {
  const approxAge = getCurrentYear() - birthYear;
  const yearsSince18 = (getCurrentYear() - birthYear) - 18;

  return [approxAge, yearsSince18];
}
```

My various attempts at doing prompt injection have failed. It seems that LLMs are pretty wise to attempts at including prompts inside the contents of files.

That's not altogether surprising. This is likely an area where there was lots of training.

Coding agents do however pay close attention to AGENTS.md files. This is something we can exploit. It appears that this works well.
