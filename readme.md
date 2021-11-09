This is the module for scanning and parsing finl input. We 
allow **commands** and **environments** to be defined.

## Input text

Input text is assumed to be in NFD normalized format.

## Commands

You indicate a command with `\` followed by either a named command or a command symbol.

Named commands consist of one or more letters. Letters are defined to include all characters in the categories letter, non-spacing mark and
combining mark. Inside user-defined macros *only*, `_` can be considered as part of a named comand

A command symbol is a single non-letter. A single non-letter is defined to include symbols which 
may be represented by multiple code 
points but a single output grapheme, For example, the symbol 🇨🇦 is actually represented by the input
sequence `U+1F1E8, U+1F1E6`. Since this is generally invisible to the user, it's most ergonomic to handle 
these sequences as if they were a single code point.

A command may have zero or more **parameters**.

### Parameters

Each parameter has a format and a type

#### Parameter formats

Parameters are one of the following formats:
* `*` an optional star indicating some alternate behavior for a command. (Corresponding to `xparse` `s`)
* optional arguments delimited with `[`…`]` (corresponding to `xparse` `o`/`O`)
* required arguments consisting either of a single token or multiple tokens delimited with `{`…`}`
  (corresponding to `xparse` `m`)
* required arguments with mandatory braces  
* arbitrary required argument where either the delimiters are `{`…`}` or will be delimited by
  the first non-space character after the command and the next occurrence of that character. This argument
  may *not* be broken across lines. (corresponding to `xparse` `v`)

Possible future enhancements:
* optional argument indicating style override delimited with `<`…`>`
* something akin to the `xparse` `r`/`R` type but allowing arbitrary delimiters (for dealing with 
  TeX-style argument handling)
* something akin to the `xparse` `d`/`D` type but allowing arbitrary delimiters to an optional argument 
  (I probably won't do this one)
* something akin to the `xparse` `e`/`E` embellishment mechanism. 

#### Parameter types

Parameters can have one of the following types:
* parsed tokens. The passed argument will be parsed and passed to the command implementer as a list of
  tokens
* verbatim text. The passed argument will be treated as verbatim and passed to the command implementer
  without further processing. Depending on the delimiter style, it may require braces or brackets to be 
  balanced within the argument, e.g., if an optional argument is treated as verbatim text, the user
  *cannot* do something like
  
      \foo[ [ ]

  I'm undecided as to whether to allow any sort of workaround for this, like

      \foo[{[}]

  or just not allow that use case at all.
* boolean values. For `*` arguments, this will simply pass `true` if present `false` if absent. Possibly
  a required or optional argument can have a boolean expression in them, although this might be something
  reserved for commands called within macro definitions.
* key-value lists. This will be a list of keys (which must conform to 
  [UAX31](http://www.unicode.org/reports/tr31/)) followed by a value with `=` separating the keys and values.
  Leading and trailing spaces will be ignored and an outer set of braces will be stripped off so that, e.g.,
  
      foo = { e } , bar =   kappa , baz=3

  will set `foo` to ` e `, `bar` to `kappa` and `baz` to `3`. All values are treated as token lists and 
  cannot contain undefined commands. If a command is found when searching for an identifier, it *must* be 
  a user-defined macro and expand to valid key-value list data.
  
  Trailing commas in the key-value list will be ignored.
* Macro definition token lists. This applies only to macro definitions. All spaces are ignored, `~` puts a 
  space into the token list, and `_` can appear in a named command. I don't know if I'll allow extensions 
  and document types to create new commands with this as a parameter type.
* Math mode.  

Possible future types:

* JSON. Perhaps with a proviso that allows dropping the enclosing braces on a JSON object, so, e.g.,

      \foo{ this: true, that: 3.1 }

  rather than

      \foo{ { this: true, that: 3.1 } }

  and perhaps also dropping brackets from an array so:

      \foo{ 1, 2, 3, 4, 5 }

  rather than

      \foo{ [1, 2, 3, 4, 5] }
  
  This will possibly also require a definition of the actual data types used in the JSON, or maybe not,
  I'm still not decided.
  
## Environments

Environments work similarly to commands but have an additional declaration of the contents of the environment.
A first parameter of `*` indicates that we're also providing for a `*`-form of the environment.

I think that environment names must conform to UAX31 (so finl would be stricter than LaTeX on this front¹).

Environment bodies can have the following types:
* Parse token list
* Math
* Verbatim²
* YAML³

---

1. This is perhaps not a big deal though—a quick `grep` on the LaTeX directory indicates that only a handful of 
   existing packages use environment names that don't conform to UAX31 with a couple having internal dashes and
   one package defining `+` versions of some math environments.
2. The finl version of the `verbatim` environment will be able to “see” the indentation of the `\end{verbatim}`
   command and strip that indentation from all lines automatically.
3. Worth remembering that YAML is a superset of JSON. The JSON shortcuts mentioned for arguments will *not* be
   supported in an environment body, but environment implementer are free to manage this themselves by taking
   input as `verbatim` and parsing it however they like.
