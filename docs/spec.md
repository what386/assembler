# Assembly Language Specification

This document defines the assembly language accepted by the assembler in this
repository. It describes the current implementation, including accepted syntax,
directive behavior, and notable limits.

The assembler is case-insensitive for instruction mnemonics, labels, assembler
directive names, register names, and condition names. Internally, those names
are normalized to lowercase.

Preprocessor define names are case-sensitive.

## Source Form

A source file is a sequence of lines. Each non-empty line may contain exactly
one statement:

- a label
- an instruction
- an assembler directive
- a preprocessor directive

Comments begin with `;` and continue to the end of the line. A `;` inside a
string literal or character literal does not begin a comment.

Blank lines are permitted anywhere.

## Lexical Syntax

### Identifiers

Identifiers begin with an ASCII letter or underscore. Remaining characters may
be ASCII letters, ASCII digits, or underscores.

Qualified names are formed by joining identifiers with `.`:

```asm
start
timer.init
pcw.clear
loop_1
```

Qualified names are used for:

- instruction mnemonics
- labels
- directive names
- preprocessor symbol names

### Integer Literals

The assembler accepts signed and unsigned integer literals in these forms:

- decimal: `0`, `42`, `255`
- hexadecimal with `0x` or `0X`: `0x10`, `0xff`
- binary with `0b` or `0B`: `0b1010`

Underscores may appear inside integer literals:

```asm
1_000
0xff_00
0b1111_0000
```

A leading `+` or `-` is parsed as a sign, not as part of the integer token.

### Character Literals

Character literals use single quotes:

```asm
'A'
'\n'
'\\'
'\''
```

Supported escape sequences are:

- `\n`
- `\r`
- `\t`
- `\0`
- `\'`
- `\"`
- `\\`

Character literals evaluate to integer values.

### String Literals

String literals use double quotes:

```asm
"text"
"line\n"
"quote: \""
```

The same escape sequences supported in character literals are supported in
string literals.

## Preprocessor

Preprocessor directives are recognized before tokenization and parsing. They
must begin with `.` at the start of the active portion of the line.

### `.define`

```asm
.define NAME replacement
```

`.define` creates a textual token replacement. The replacement must not be
empty.

Examples:

```asm
.define VALUE 42
.define PTR [r3+1]
```

Rules:

- define names are case-sensitive
- redefining an existing name is an error
- recursive define expansion is an error
- replacement happens over tokens, not raw text

### Conditional Assembly

```asm
.ifdef NAME
.ifndef NAME
.else
.endif
```

Rules:

- `.ifdef NAME` is active when `NAME` is defined
- `.ifndef NAME` is active when `NAME` is not defined
- `.else` may appear at most once per conditional block
- `.endif` must close a matching `.ifdef` or `.ifndef`
- unterminated conditionals are errors

Inactive branches are removed before parsing.

### Command-Line Defines

The CLI option `-Dname[=value]` creates a preprocessor definition before the
source file is processed.

Examples:

```text
assembler -DFLAG input.asm
assembler -DVALUE=42 input.asm
```

If `=value` is omitted, the replacement value is `1`.

## Statements

### Labels

A label is a qualified name followed by `:`:

```asm
start:
loop.inner:
```

Labels define the current output address. Defining the same label more than
once is an error.

### Instructions

An instruction consists of a mnemonic followed by zero or more operands:

```asm
halt
lim r0, 0x10
bra loop, ?lower
timer.init 5
```

Operands are comma-separated. A trailing comma is an error.

### Directives

An assembler directive begins with `.` followed by a qualified name:

```asm
.page 1
.org 0xff00
.string "text"
```

Directive argument parsing rules:

- `.bytes` uses comma-separated arguments
- all other directives use positional whitespace-separated arguments
- commas in non-`.bytes` directives are errors

## Operands

### Registers

The only register names are:

```text
r0 r1 r2 r3 r4 r5 r6 r7
```

### Immediate Operands

An immediate operand is:

- an integer literal, optionally signed
- a character literal

Examples:

```asm
lim r0, 10
addi r1, -1
cmpi r2, 'A'
```

### Label Operands

A bare qualified name that is not a register is parsed as a label operand:

```asm
jmp start
cal handler.entry
bra done, ?equal
```

Location operands are accepted for `jmp`, `cal`, and `bra`.

### Conditions

Standard conditions use `?name`:

```asm
?equal
?not_equal
?lower
```

Accepted standard condition names are:

- `equal`
- `zero`
- `not_equal`
- `not_zero`
- `lower`
- `higher`
- `lower_same`
- `higher_same`
- `carry`
- `even`
- `always`

Alternate conditions use `@name`:

```asm
@overflow
@greater_equal
```

Accepted alternate condition names are:

- `overflow`
- `no_overflow`
- `less`
- `greater`
- `less_equal`
- `greater_equal`
- `odd`
- `always`

### Address Operands

The assembler accepts two address forms.

Absolute addresses:

```asm
[0x02]
[255]
```

Rules:

- the value must be non-negative
- the value is encoded as an absolute address when the instruction expects one

Indexed addresses:

```asm
[r3]
[r3+4]
[r3-1]
```

Rules:

- the base must be a register
- the parsed offset must fit in `i8`
- some instructions require no offset, while others allow one
- encoding may impose a stricter range than parsing for some indexed forms

## Assembler Directives

### `.page <n>`

```asm
.page 0
.page 2
```

`.page` moves the output cursor to the start of page `n`. Pages are 128 bytes
wide.

Current behavior:

- pages are laid out in a flat binary image
- moving backward is not permitted
- padding between the old cursor and the page start is zero-filled
- a page region may not exceed 128 bytes
- `bra` targets must remain within the same 64-instruction page

### `.org <addr>`

```asm
.org 0xff00
```

`.org` moves the output cursor to an absolute byte address in the output image.

Current behavior:

- moving backward is not permitted
- padding between the old cursor and the new address is zero-filled

### `.bytes <b0>, <b1>, ...`

```asm
.bytes 0x00, 0x1c, 0xff
.bytes 'A', '\n'
```

Rules:

- arguments must be integer or character literals
- each value must fit in 8 bits

### `.string "text"`

```asm
.string "text"
```

`.string` emits the raw bytes of the string literal. No terminator is added.

### `.zero <count>`

```asm
.zero 16
```

`.zero` emits `count` zero bytes.

Rules:

- `count` must be an integer or character literal
- `count` must be non-negative

## Instruction Set Surface

This section lists the mnemonics currently accepted by the assembler. Operand
shapes are described informally; validation is enforced by the assembler.

### Core Mnemonics

| Mnemonic | Accepted Form |
| --- | --- |
| `in` | `in reg, [abs]` |
| `out` | `out [abs], reg` |
| `jmp` | `jmp location` |
| `bra` | `bra location, condition` |
| `cal` | `cal location` |
| `crets` | `crets condition, imm` |
| `blit` | `blit [reg], [reg]` |
| `bit` | `bit reg, reg, imm` |
| `pop` | `pop reg, imm` |
| `psh` | `psh reg, imm` |
| `mld` | `mld reg, [abs]` |
| `mst` | `mst [abs], reg` |
| `mlx` | `mlx reg, [reg+off]` |
| `msx` | `msx [reg+off], reg` |
| `lim` | `lim reg, imm` |
| `cmov` | `cmov reg, reg, condition` |
| `addi` | `addi reg, imm` |
| `andi` | `andi reg, imm` |
| `ori` | `ori reg, imm` |
| `xori` | `xori reg, imm` |
| `cmpi` | `cmpi reg, imm` |
| `tsti` | `tsti reg, imm` |
| `add` | `add reg, reg, reg` |
| `sub` | `sub reg, reg, reg` |
| `bitw` | `bitw reg, reg, reg` |
| `bntw` | `bntw reg, reg, reg` |
| `bsh` | `bsh reg, reg, reg` |
| `bshi` | `bshi reg, reg, imm` |
| `mdo` | `mdo reg, reg, reg` |
| `btc` | `btc reg, reg, imm` |
| `func` | reserved encoding form; not accepted directly |
| `ctrl` | reserved encoding form; not accepted directly |

### Pseudo-Instructions

These mnemonics are accepted directly:

- `brx`
- `cmp`
- `not`
- `inc`
- `dec`
- `halt`
- `nop`

### Aliases

These mnemonics are accepted and resolve to canonical instruction forms:

- return / trap aliases: `ret`, `brk`, `iret`
- stack aliases: `peek`, `popf`, `dsp`, `poke`, `pshf`, `isp`
- move / exchange aliases: `mov`, `xchg`
- arithmetic aliases: `adc`, `adv`, `advc`, `sbb`, `sbv`, `sbvb`
- bitwise aliases: `and`, `or`, `xor`, `imp`, `nand`, `nor`, `xnor`, `nimp`
- shift aliases: `bsl`, `bsr`, `rol`, `bsxr`, `bsli`, `bsri`, `roli`, `bsxri`
- multiply / divide aliases: `mul`, `mulu`, `div`, `mod`
- bit-count aliases: `sqrt`, `clz`, `ctz`, `popcnt`
- function / control aliases: `mpge`, `int`, `timer.init`, `timer.val`,
  `pcw.clear`, `pcw.set`

## Current Limits and Behavior Notes

- Output is always a flat binary image.
- Successful assembly is silent unless `-E` is used or diagnostics are printed.
- The CLI option `-o <file>` writes the final binary image.
- The CLI option `-E` prints preprocessed output instead of assembling.
- Unknown directives may parse successfully, but encoding them is an error unless
  they are explicitly supported.

## Example

```asm
.page 0
start:
    lim r0, 0x10
    mld r1, [0x02]
    bra start, ?not_equal

.org 0xff00
data:
    .string "text"
    .bytes 0x00, 0x1c, 0xff
```
