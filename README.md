# Marzipan
Marzipan is a macro processor for [ZZT](https://en.wikipedia.org/wiki/ZZT).
It extends the ZZT-OOP scripting language while still maintaining compatibility with ZZT 3.2: all language features compile down to vanilla ZZT-OOP.

## Features
- **Anonymous labels:** Like assembly code, ZZT-OOP control flow relies heavily on gotos.
    Anonymous labels reduce the cognitive overhead of writing such code.
    `:@` defines a label with a unique name, and `@f`/`@b` reference the nearest anonymous label forward/backward of the current line.
- **Local labels:** Label names with a dot are scoped to a single section of an object's program.
    This allows you to reuse a name like `.loop` multiple times in a single object's code.
    Marzipan will replace this with a distinct name per section: `loop_` in one section, `loopa` in the next section, etc.
- **Macro language (WIP):** Lines starting with `%` invoke a Marzipan macro.
    Macros work by text substitution; for example, `%include "foo.txt"` will insert the contents of a text file at the current line.

Here's an example of what anonymous labels look like in practice:

```
@Treasure chest
#end
:touch                  'Event handler, user touched the chest.
#if key @f              'Jump to anonymous label. Marzipan compiles `@f` to `_`...
The chest is locked.
#end
:@                      '...and compiles this to `:_`.
You unlock the chest.
There's a bunch of gems inside!
#give gems 20
#die

:shot                   'Event handler, user shot at the chest.
#if shotgun @f          'This time, Marzipan compiles `@f` to `a`...
Nothing happens.
#end
:@                      '...and compiles this to `:a`.
The chest shatters!
Gems fly everywhere.
#put w green gem
#put e green gem
#become green gem
```

## Usage
Marzipan reads and writes ZZT world files, a binary file format.
You will need either ZZT itself or an external ZZT editor (such as [KevEdit](https://github.com/cknave/kevedit)) to work with them.

```
marzipan WORLD.ZZT -o ./dest_folder/WORLD.ZZT
```

A disclaimer: **Marzipan is experimental.**
It hasn't eaten my code yet, but I cannot guarantee it will treat your code with kindness.
If you use it, make sure to keep backups of your work. (You were already keeping backups, right?)

## Planned features
- Extending the macro system to a full language with variables, custom macro definitions, etc.
- Code minification, for generated code that bumps up against ZZT 3.2's size limits.
- Lints for ZZT-OOP, such as dead code analysis.
