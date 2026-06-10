# LLVM Backend Feasibility Findings

**Date:** 2026-06-10T10:23:35Z
**Status:** SKIP — LLVM backend kompile failed.
**Script:** contrib/kloom/scripts/kloom-llvm-feasibility.sh

## Result

`kompile --backend llvm` failed with the following output:

```text
[Warning] Compiler: Variable 'N' defined but not used. Prefix variable name
with underscore if this is intentional.
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(132,19,132,20)
	132 |	  rule <k> TypeOf(N:Int) => TypeOfResult(int64Ty) ... </k>
	    .	                  ^
[Error] Compiler: Found variable StartVal on right hand side of rule, not bound
on left hand side. Did you mean "?StartVal"?
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(501,39,501,47)
	501 |	           => ExecForRangeLoop(Index, StartVal, EndVal, Body, Saved, 0,
MaxR)
	    .	                                      ^~~~~~~~
[Error] Compiler: Found variable EndVal on right hand side of rule, not bound
on left hand side. Did you mean "?EndVal"?
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(501,49,501,55)
	501 |	           => ExecForRangeLoop(Index, StartVal, EndVal, Body, Saved, 0,
MaxR)
	    .	                                                ^~~~~~
[Error] Compiler: Found variable StartVal on right hand side of rule, not bound
on left hand side. Did you mean "?StartVal"?
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(507,44,507,52)
	507 |	       requires EvalConst(StartExpr) ==Int StartVal
	    .	                                           ^~~~~~~~
[Error] Compiler: Found variable EndVal on right hand side of rule, not bound
on left hand side. Did you mean "?EndVal"?
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(508,42,508,48)
	508 |	        andBool EvalConst(EndExpr) ==Int EndVal
	    .	                                         ^~~~~~
[Warning] Compiler: Variable 'Index' defined but not used. Prefix variable name
with underscore if this is intentional.
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(520,29,520,34)
	520 |	  rule <k> ExecForRangeLoop(Index, Cur, End, Body, Saved, Count, Max) =>
.K ... </k>
	    .	                            ^~~~~
[Warning] Compiler: Variable 'Body' defined but not used. Prefix variable name
with underscore if this is intentional.
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(520,46,520,50)
	520 |	  rule <k> ExecForRangeLoop(Index, Cur, End, Body, Saved, Count, Max) =>
.K ... </k>
	    .	                                             ^~~~
[Warning] Compiler: Variable 'Count' defined but not used. Prefix variable name
with underscore if this is intentional.
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(520,59,520,64)
	520 |	  rule <k> ExecForRangeLoop(Index, Cur, End, Body, Saved, Count, Max) =>
.K ... </k>
	    .	                                                          ^~~~~
[Warning] Compiler: Variable 'Max' defined but not used. Prefix variable name
with underscore if this is intentional.
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(520,66,520,69)
	520 |	  rule <k> ExecForRangeLoop(Index, Cur, End, Body, Saved, Count, Max) =>
.K ... </k>
	    .	                                                                 ^~~
[Warning] Compiler: Variable 'M' defined but not used. Prefix variable name
with underscore if this is intentional.
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(526,18,526,19)
	526 |	       <scalars> M:Map </scalars>
	    .	                 ^
[Warning] Compiler: Variable 'M' defined but not used. Prefix variable name
with underscore if this is intentional.
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(535,18,535,19)
	535 |	       <scalars> M:Map </scalars>
	    .	                 ^
[Warning] Compiler: Variable 'M' defined but not used. Prefix variable name
with underscore if this is intentional.
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(550,55,550,56)
	550 |	  rule IsMonotoneProgress(Cursor, add(Cursor, N:Int), M:Map) => true
	    .	                                                      ^
[Error] Compiler: Found variable LimitVal on right hand side of rule, not bound
on left hand side. Did you mean "?LimitVal"?
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(557,42,557,50)
	557 |	           => ExecCursorLoopLoop(Cursor, LimitVal, Body, Saved, 0, MaxR)
	    .	                                         ^~~~~~~~
[Error] Compiler: Found variable LimitVal on right hand side of rule, not bound
on left hand side. Did you mean "?LimitVal"?
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(563,44,563,52)
	563 |	       requires EvalConst(LimitExpr) ==Int LimitVal
	    .	                                           ^~~~~~~~
[Warning] Compiler: Variable 'Cursor' defined but not used. Prefix variable
name with underscore if this is intentional.
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(575,31,575,37)
	575 |	  rule <k> ExecCursorLoopLoop(Cursor, Remaining, Body, Saved, Count,
Max) => .K ... </k>
	    .	                              ^~~~~~
[Warning] Compiler: Variable 'Body' defined but not used. Prefix variable name
with underscore if this is intentional.
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(575,50,575,54)
	575 |	  rule <k> ExecCursorLoopLoop(Cursor, Remaining, Body, Saved, Count,
Max) => .K ... </k>
	    .	                                                 ^~~~
[Warning] Compiler: Variable 'Count' defined but not used. Prefix variable name
with underscore if this is intentional.
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(575,63,575,68)
	575 |	  rule <k> ExecCursorLoopLoop(Cursor, Remaining, Body, Saved, Count,
Max) => .K ... </k>
	    .	                                                              ^~~~~
[Warning] Compiler: Variable 'Max' defined but not used. Prefix variable name
with underscore if this is intentional.
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(575,70,575,73)
	575 |	  rule <k> ExecCursorLoopLoop(Cursor, Remaining, Body, Saved, Count,
Max) => .K ... </k>
	    .	                                                                     ^~~
[Warning] Compiler: Variable 'M' defined but not used. Prefix variable name
with underscore if this is intentional.
	Source(/Users/macintoshhd/loom-demo/contrib/kloom/src/kloom.k)
	Location(588,18,588,19)
	588 |	       <scalars> M:Map </scalars>
	    .	                 ^
[Error] Compiler: Had 6 structural errors.
[Warning] Compiler: Could not find main syntax module with name KLOOM-SYNTAX in
definition.  Use --syntax-module to specify one. Using KLOOM as default.
```

This is a recorded skip, not a failure.

## Explicit Unknowns (A1–A4)

- **A1**: Whether K LLVM backend supports all builtins used in kloom.k
  (INT, BOOL, LIST, MAP, STRING).
- **A2**: Whether `krun --output pretty` with LLVM backend produces the same
  `<events>` cell format as Haskell backend.
- **A3**: Whether `nix profile install nixpkgs#kframework` on nixos-unstable
  includes the LLVM backend toolchain.
- **A4**: Whether `kore-exec.tar.gz` is a Haskell-backend artifact and not
  reusable for LLVM backend.

## Next Steps

Investigate kompile error and retry, or verify LLVM backend availability
in the installed K Framework version.
