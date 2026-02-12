# Concepts

Defeasible logic is a **non-monotonic reasoning** system. This chapter introduces the core concepts you need to understand Spindle.

## Classical vs. Defeasible Logic

**Classical (Monotonic) Logic:**
- Once proven, always proven
- Adding facts only adds conclusions
- Cannot handle exceptions

**Defeasible (Non-Monotonic) Logic:**
- Conclusions are tentative
- New evidence can defeat existing conclusions
- Handles exceptions naturally

## The Tweety Problem

The motivating example for defeasible logic:

> Tweety is a bird. Tweety is a penguin. Birds fly. Penguins don't fly. Does Tweety fly?

Classical logic produces a contradiction. Defeasible logic resolves it by recognizing that "penguins don't fly" is a more specific rule that should override "birds fly."

```lisp
(given bird)
(given penguin)
(normally r1 bird flies)
(normally r2 penguin (not flies))
(prefer r2 r1)
```

Result: Tweety doesn't fly.

## Key Terminology

| Term | Definition |
|------|------------|
| **Literal** | An atomic proposition, possibly negated (e.g., `flies`, `-flies`) |
| **Rule** | A conditional statement with body and head |
| **Theory** | A collection of rules and superiority relations |
| **Conclusion** | A proven literal with a provability level |
| **Defeat** | When one rule blocks another's conclusion |
| **Superiority** | A preference relation between rules |

## Chapters

- [Rules and Facts](concepts/rules.md) - The four rule types
- [Conclusions](concepts/conclusions.md) - Understanding +D, -D, +d, -d
- [Superiority](concepts/superiority.md) - Resolving conflicts
- [Negation](concepts/negation.md) - Strong negation in Spindle
