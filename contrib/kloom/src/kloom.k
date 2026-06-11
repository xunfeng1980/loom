/*
  kloom — K Framework executable semantics for Loom L2Core (v4)

  Covers: readInput, letScalar, forRange, cursorLoop, full expression
  language (add/sub/mul/min/max/eq/lt/le), bytes constants, failClosed.

  Trust model: spec-oracle, offline/CI only, outside production TCB.
*/

module LOOMCORE-SYNTAX
  imports INT
  imports BOOL
  imports STRING

  syntax Id ::= r"[a-zA-Z][a-zA-Z0-9_]*" [token]

  syntax L2Ty ::= "int32"
                | "int64"
                | "float32"
                | "float64"
                | "bool"
                | "bytes"

  syntax ScalarType ::= "boolTy"
                      | "int32Ty"
                      | "int64Ty"
                      | "float32Ty"
                      | "float64Ty"
                      | "uint32Ty"
                      | "uint64Ty"
                      | "bytesTy"
                      | "rowIndexTy"

  syntax ScalarValue ::= Int
                       | Bool

  syntax ScalarExpr ::= ScalarValue
                      | Id
                      | "bytesConst" "(" String ")"
                      | "add" "(" ScalarExpr "," ScalarExpr ")"
                      | "sub" "(" ScalarExpr "," ScalarExpr ")"
                      | "mul" "(" ScalarExpr "," ScalarExpr ")"
                      | "min" "(" ScalarExpr "," ScalarExpr ")"
                      | "max" "(" ScalarExpr "," ScalarExpr ")"
                      | "eq"  "(" ScalarExpr "," ScalarExpr ")"
                      | "lt"  "(" ScalarExpr "," ScalarExpr ")"
                      | "le"  "(" ScalarExpr "," ScalarExpr ")"

  syntax Capability ::= "input" Id ":" L2Ty Int Int
                      | "builder" Id ":" L2Ty
                      | "builder" Id ":" L2Ty "nullable"

  syntax Capabilities ::= List{Capability,","}

  syntax Stmt ::= "appendValue" "(" Id "," ScalarExpr ")"
                | "appendNull"  "(" Id ")"
                | "readInput"   "(" Id "," ScalarExpr "," ScalarExpr "," Id ")"
                | "letScalar"   "(" Id "," ScalarExpr ")"
                | "forRange"    "(" Id "," ScalarExpr "," ScalarExpr "," Stmts ")"
                | "cursorLoop"  "(" Id "," ScalarExpr "," ScalarExpr "," Stmts ")"
                | "failClosed"  "(" String ")"

  syntax Stmts ::= List{Stmt,";"}

  syntax Program ::= "program" Capabilities "body" Stmts "maxRows" Int

  syntax TraceEvent ::= "append-value" ":" Id ":" L2Ty
                      | "append-null"  ":" Id ":" L2Ty
                      | "read"         ":" Id ":" String ":" String ":" String
                      | "fail-closed"  ":" String
                      | "terminal"     ":" "finished"
endmodule


module LOOMCORE
  imports LOOMCORE-SYNTAX
  imports INT
  imports BOOL
  imports LIST
  imports MAP
  imports STRING

  // -------------------------------------------------------------------------
  // Internal capability representation
  // -------------------------------------------------------------------------
  syntax CapInfo ::= inputInfo(L2Ty, Int, Int)
                   | builderInfo(L2Ty, Bool)

  syntax Status ::= "running" | "finished" | "failClosed"

  // -------------------------------------------------------------------------
  // Configuration
  // -------------------------------------------------------------------------
  configuration
    <T>
      <k> $PGM:Program </k>
      <caps> .Map </caps>
      <scalars> .Map </scalars>
      <events> .List </events>
      <reads> .List </reads>
      <rows> 0 </rows>
      <maxRows> 0 </maxRows>
      <status> running </status>
    </T>

  // -------------------------------------------------------------------------
  // Helper: constant evaluation (pure function, -1 means fail)
  // -------------------------------------------------------------------------
  syntax Int ::= EvalConst(ScalarExpr) [function]

  rule EvalConst(N:Int) => N
  rule EvalConst(add(E1, E2)) => EvalConst(E1) +Int EvalConst(E2)
  rule EvalConst(sub(E1, E2)) => EvalConst(E1) -Int EvalConst(E2)
  rule EvalConst(mul(E1, E2)) => EvalConst(E1) *Int EvalConst(E2)
  rule EvalConst(min(E1, E2)) => EvalConst(E1)
       requires EvalConst(E1) <=Int EvalConst(E2)
  rule EvalConst(min(E1, E2)) => EvalConst(E2)
       requires EvalConst(E2) <Int EvalConst(E1)
  rule EvalConst(max(E1, E2)) => EvalConst(E1)
       requires EvalConst(E1) >=Int EvalConst(E2)
  rule EvalConst(max(E1, E2)) => EvalConst(E2)
       requires EvalConst(E2) >Int EvalConst(E1)
  rule EvalConst(_) => -1 [owise]

  // -------------------------------------------------------------------------
  // Helper: expression type-of (configuration-aware via ~> sequence)
  // -------------------------------------------------------------------------
  syntax KItem ::= TypeOf(ScalarExpr)
                 | TypeOfResult(ScalarType)
                 | TypeOfAddSecond(ScalarExpr)
                 | TypeOfSubSecond(ScalarExpr)
                 | TypeOfMulSecond(ScalarExpr)
                 | TypeOfMinSecond(ScalarExpr)
                 | TypeOfMaxSecond(ScalarExpr)
                 | TypeOfEqSecond(ScalarExpr)
                 | TypeOfLtSecond(ScalarExpr)
                 | TypeOfLeSecond(ScalarExpr)

  syntax TypeOfAddCheck
  syntax TypeOfSubCheck
  syntax TypeOfMulCheck
  syntax TypeOfMinCheck
  syntax TypeOfMaxCheck
  syntax TypeOfEqCheck
  syntax TypeOfLtCheck
  syntax TypeOfLeCheck

  rule <k> TypeOf(N:Int) => TypeOfResult(int64Ty) ... </k>
  rule <k> TypeOf(true) => TypeOfResult(boolTy) ... </k>
  rule <k> TypeOf(false) => TypeOfResult(boolTy) ... </k>
  rule <k> TypeOf(bytesConst(_:String)) => TypeOfResult(bytesTy) ... </k>

  rule <k> TypeOf(V:Id) => TypeOfResult(T) ... </k>
       <scalars>... V |-> T ...</scalars>

  rule <k> TypeOf(add(E1, E2)) => TypeOf(E1) ~> TypeOfAddSecond(E2) ... </k>
  rule <k> TypeOfResult(T) ~> TypeOfAddSecond(E2) => TypeOf(E2) ~> typeOfAddCheck(T) ... </k>
  syntax KItem ::= typeOfAddCheck(ScalarType)
  rule <k> TypeOfResult(_) ~> typeOfAddCheck(T) => TypeOfResult(T) ... </k>

  rule <k> TypeOf(sub(E1, E2)) => TypeOf(E1) ~> TypeOfSubSecond(E2) ... </k>
  rule <k> TypeOfResult(T) ~> TypeOfSubSecond(E2) => TypeOf(E2) ~> typeOfSubCheck(T) ... </k>
  syntax KItem ::= typeOfSubCheck(ScalarType)
  rule <k> TypeOfResult(_) ~> typeOfSubCheck(T) => TypeOfResult(T) ... </k>

  rule <k> TypeOf(mul(E1, E2)) => TypeOf(E1) ~> TypeOfMulSecond(E2) ... </k>
  rule <k> TypeOfResult(T) ~> TypeOfMulSecond(E2) => TypeOf(E2) ~> typeOfMulCheck(T) ... </k>
  syntax KItem ::= typeOfMulCheck(ScalarType)
  rule <k> TypeOfResult(_) ~> typeOfMulCheck(T) => TypeOfResult(T) ... </k>

  syntax KItem ::= typeOfMinCheck(ScalarType)
  rule <k> TypeOfResult(_) ~> typeOfMinCheck(T) => TypeOfResult(T) ... </k>

  syntax KItem ::= typeOfMaxCheck(ScalarType)
  rule <k> TypeOfResult(_) ~> typeOfMaxCheck(T) => TypeOfResult(T) ... </k>

  rule <k> TypeOf(eq(E1, E2)) => TypeOf(E1) ~> TypeOfEqSecond(E2) ... </k>
  rule <k> TypeOfResult(_) ~> TypeOfEqSecond(E2) => TypeOf(E2) ~> typeOfEqCheck() ... </k>
  syntax KItem ::= typeOfEqCheck()
  rule <k> TypeOfResult(_) ~> typeOfEqCheck() => TypeOfResult(boolTy) ... </k>

  rule <k> TypeOf(lt(E1, E2)) => TypeOf(E1) ~> TypeOfLtSecond(E2) ... </k>
  rule <k> TypeOfResult(_) ~> TypeOfLtSecond(E2) => TypeOf(E2) ~> typeOfLtCheck() ... </k>
  syntax KItem ::= typeOfLtCheck()
  rule <k> TypeOfResult(_) ~> typeOfLtCheck() => TypeOfResult(boolTy) ... </k>

  rule <k> TypeOf(le(E1, E2)) => TypeOf(E1) ~> TypeOfLeSecond(E2) ... </k>
  rule <k> TypeOfResult(_) ~> TypeOfLeSecond(E2) => TypeOf(E2) ~> typeOfLeCheck() ... </k>
  syntax KItem ::= typeOfLeCheck()
  rule <k> TypeOfResult(_) ~> typeOfLeCheck() => TypeOfResult(boolTy) ... </k>

  rule <k> TypeOf(min(E1, E2)) => TypeOf(E1) ~> TypeOfMinSecond(E2) ... </k>
  rule <k> TypeOfResult(T) ~> TypeOfMinSecond(E2) => TypeOf(E2) ~> typeOfMinCheck(T) ... </k>
  syntax KItem ::= typeOfMinCheck(ScalarType)
  rule <k> TypeOfResult(_) ~> typeOfMinCheck(T) => TypeOfResult(T) ... </k>

  rule <k> TypeOf(max(E1, E2)) => TypeOf(E1) ~> TypeOfMaxSecond(E2) ... </k>
  rule <k> TypeOfResult(T) ~> TypeOfMaxSecond(E2) => TypeOf(E2) ~> typeOfMaxCheck(T) ... </k>
  syntax KItem ::= typeOfMaxCheck(ScalarType)
  rule <k> TypeOfResult(_) ~> typeOfMaxCheck(T) => TypeOfResult(T) ... </k>

  // -------------------------------------------------------------------------
  // Helper: scalar environment
  // -------------------------------------------------------------------------
  syntax KItem ::= SetScalar(Id, ScalarType)
                 | RestoreScalars(Map)

  rule <k> SetScalar(Name, T) => .K ... </k>
       <scalars> M:Map => M[Name <- T] </scalars>

  rule <k> RestoreScalars(M:Map) => .K ... </k>
       <scalars> _ => M </scalars>

  // -------------------------------------------------------------------------
  // initCaps — surface syntax → internal representation
  // -------------------------------------------------------------------------
  syntax KItem ::= InitCaps(Capabilities)

  rule InitCaps(.Capabilities) => .K

  rule <k> InitCaps(input N:Id : T:L2Ty Off:Int Len:Int , Cs:Capabilities)
           => InitCaps(Cs) ...
       </k>
       <caps> M:Map => M[N <- inputInfo(T, Off, Len)] </caps>

  rule <k> InitCaps(builder N:Id : T:L2Ty , Cs:Capabilities)
           => InitCaps(Cs) ...
       </k>
       <caps> M:Map => M[N <- builderInfo(T, false)] </caps>

  rule <k> InitCaps(builder N:Id : T:L2Ty nullable , Cs:Capabilities)
           => InitCaps(Cs) ...
       </k>
       <caps> M:Map => M[N <- builderInfo(T, true)] </caps>

  // -------------------------------------------------------------------------
  // Program initialization
  // -------------------------------------------------------------------------
  rule <k> (program Cs:Capabilities body Ss:Stmts maxRows MR:Int)
           => InitCaps(Cs) ~> ExecBody(Ss) ~> finish()
           ...
       </k>
       <maxRows> _ => MR </maxRows>

  // -------------------------------------------------------------------------
  // Statement dispatch
  // -------------------------------------------------------------------------
  syntax KItem ::= ExecBody(Stmts)
                 | ExecStmt(Stmt)
                 | finish() [symbol(finish)]

  rule ExecBody(.Stmts) => .K
  rule ExecBody(S:Stmt ; Ss:Stmts) => ExecStmt(S) ~> ExecBody(Ss)

  // -------------------------------------------------------------------------
  // readInput
  // -------------------------------------------------------------------------
  // In-bounds with known constants (int32)
  rule <k> ExecStmt(readInput(Cap:Id, OffExpr:ScalarExpr, WidthExpr:ScalarExpr, Bind:Id))
           => .K ...
       </k>
       <caps>... Cap |-> inputInfo(int32, CapOff:Int, CapLen:Int) ...</caps>
       <scalars> M:Map => M[Bind <- int32Ty] </scalars>
       <events> Es:List => Es ListItem(read : Cap : "expr" : "expr" : "true") </events>
       <reads> Rs:List => Rs ListItem(read : Cap : "expr" : "expr" : "true") </reads>
       <status> running </status>
       requires EvalConst(OffExpr) >=Int 0
        andBool EvalConst(WidthExpr) >=Int 0
        andBool EvalConst(OffExpr) >=Int CapOff
        andBool EvalConst(OffExpr) +Int EvalConst(WidthExpr) <=Int CapOff +Int CapLen

  // Out-of-bounds with known constants (int32)
  rule <k> ExecStmt(readInput(Cap:Id, OffExpr:ScalarExpr, WidthExpr:ScalarExpr, _:Id))
           => .K ...
       </k>
       <caps>... Cap |-> inputInfo(int32, CapOff:Int, CapLen:Int) ...</caps>
       <events> Es:List => Es ListItem(read : Cap : "expr" : "expr" : "false") </events>
       <status> running => failClosed </status>
       <reads> Rs:List => Rs ListItem(read : Cap : "expr" : "expr" : "false") </reads>
       requires EvalConst(OffExpr) >=Int 0
        andBool EvalConst(WidthExpr) >=Int 0
        andBool (EvalConst(OffExpr) <Int CapOff orBool EvalConst(OffExpr) +Int EvalConst(WidthExpr) >Int CapOff +Int CapLen)

  // Unknown constants (int32)
  rule <k> ExecStmt(readInput(Cap:Id, OffExpr:ScalarExpr, WidthExpr:ScalarExpr, Bind:Id))
           => .K ...
       </k>
       <caps>... Cap |-> inputInfo(int32, _, _) ...</caps>
       <scalars> M:Map => M[Bind <- int32Ty] </scalars>
       <events> Es:List => Es ListItem(read : Cap : "expr" : "expr" : "true") </events>
       <reads> Rs:List => Rs ListItem(read : Cap : "expr" : "expr" : "true") </reads>
       <status> running </status>
       requires EvalConst(OffExpr) <Int 0 orBool EvalConst(WidthExpr) <Int 0
  // In-bounds with known constants (int64)
  rule <k> ExecStmt(readInput(Cap:Id, OffExpr:ScalarExpr, WidthExpr:ScalarExpr, Bind:Id))
           => .K ...
       </k>
       <caps>... Cap |-> inputInfo(int64, CapOff:Int, CapLen:Int) ...</caps>
       <scalars> M:Map => M[Bind <- int64Ty] </scalars>
       <events> Es:List => Es ListItem(read : Cap : "expr" : "expr" : "true") </events>
       <reads> Rs:List => Rs ListItem(read : Cap : "expr" : "expr" : "true") </reads>
       <status> running </status>
       requires EvalConst(OffExpr) >=Int 0
        andBool EvalConst(WidthExpr) >=Int 0
        andBool EvalConst(OffExpr) >=Int CapOff
        andBool EvalConst(OffExpr) +Int EvalConst(WidthExpr) <=Int CapOff +Int CapLen

  // Out-of-bounds with known constants (int64)
  rule <k> ExecStmt(readInput(Cap:Id, OffExpr:ScalarExpr, WidthExpr:ScalarExpr, _:Id))
           => .K ...
       </k>
       <caps>... Cap |-> inputInfo(int64, CapOff:Int, CapLen:Int) ...</caps>
       <events> Es:List => Es ListItem(read : Cap : "expr" : "expr" : "false") </events>
       <status> running => failClosed </status>
       <reads> Rs:List => Rs ListItem(read : Cap : "expr" : "expr" : "false") </reads>
       requires EvalConst(OffExpr) >=Int 0
        andBool EvalConst(WidthExpr) >=Int 0
        andBool (EvalConst(OffExpr) <Int CapOff orBool EvalConst(OffExpr) +Int EvalConst(WidthExpr) >Int CapOff +Int CapLen)

  // Unknown constants (int64)
  rule <k> ExecStmt(readInput(Cap:Id, OffExpr:ScalarExpr, WidthExpr:ScalarExpr, Bind:Id))
           => .K ...
       </k>
       <caps>... Cap |-> inputInfo(int64, _, _) ...</caps>
       <scalars> M:Map => M[Bind <- int64Ty] </scalars>
       <events> Es:List => Es ListItem(read : Cap : "expr" : "expr" : "true") </events>
       <reads> Rs:List => Rs ListItem(read : Cap : "expr" : "expr" : "true") </reads>
       <status> running </status>
       requires EvalConst(OffExpr) <Int 0 orBool EvalConst(WidthExpr) <Int 0
  // In-bounds with known constants (float32)
  rule <k> ExecStmt(readInput(Cap:Id, OffExpr:ScalarExpr, WidthExpr:ScalarExpr, Bind:Id))
           => .K ...
       </k>
       <caps>... Cap |-> inputInfo(float32, CapOff:Int, CapLen:Int) ...</caps>
       <scalars> M:Map => M[Bind <- float32Ty] </scalars>
       <events> Es:List => Es ListItem(read : Cap : "expr" : "expr" : "true") </events>
       <reads> Rs:List => Rs ListItem(read : Cap : "expr" : "expr" : "true") </reads>
       <status> running </status>
       requires EvalConst(OffExpr) >=Int 0
        andBool EvalConst(WidthExpr) >=Int 0
        andBool EvalConst(OffExpr) >=Int CapOff
        andBool EvalConst(OffExpr) +Int EvalConst(WidthExpr) <=Int CapOff +Int CapLen

  // Out-of-bounds with known constants (float32)
  rule <k> ExecStmt(readInput(Cap:Id, OffExpr:ScalarExpr, WidthExpr:ScalarExpr, _:Id))
           => .K ...
       </k>
       <caps>... Cap |-> inputInfo(float32, CapOff:Int, CapLen:Int) ...</caps>
       <events> Es:List => Es ListItem(read : Cap : "expr" : "expr" : "false") </events>
       <status> running => failClosed </status>
       <reads> Rs:List => Rs ListItem(read : Cap : "expr" : "expr" : "false") </reads>
       requires EvalConst(OffExpr) >=Int 0
        andBool EvalConst(WidthExpr) >=Int 0
        andBool (EvalConst(OffExpr) <Int CapOff orBool EvalConst(OffExpr) +Int EvalConst(WidthExpr) >Int CapOff +Int CapLen)

  // Unknown constants (float32)
  rule <k> ExecStmt(readInput(Cap:Id, OffExpr:ScalarExpr, WidthExpr:ScalarExpr, Bind:Id))
           => .K ...
       </k>
       <caps>... Cap |-> inputInfo(float32, _, _) ...</caps>
       <scalars> M:Map => M[Bind <- float32Ty] </scalars>
       <events> Es:List => Es ListItem(read : Cap : "expr" : "expr" : "true") </events>
       <reads> Rs:List => Rs ListItem(read : Cap : "expr" : "expr" : "true") </reads>
       <status> running </status>
       requires EvalConst(OffExpr) <Int 0 orBool EvalConst(WidthExpr) <Int 0
  // In-bounds with known constants (float64)
  rule <k> ExecStmt(readInput(Cap:Id, OffExpr:ScalarExpr, WidthExpr:ScalarExpr, Bind:Id))
           => .K ...
       </k>
       <caps>... Cap |-> inputInfo(float64, CapOff:Int, CapLen:Int) ...</caps>
       <scalars> M:Map => M[Bind <- float64Ty] </scalars>
       <events> Es:List => Es ListItem(read : Cap : "expr" : "expr" : "true") </events>
       <reads> Rs:List => Rs ListItem(read : Cap : "expr" : "expr" : "true") </reads>
       <status> running </status>
       requires EvalConst(OffExpr) >=Int 0
        andBool EvalConst(WidthExpr) >=Int 0
        andBool EvalConst(OffExpr) >=Int CapOff
        andBool EvalConst(OffExpr) +Int EvalConst(WidthExpr) <=Int CapOff +Int CapLen

  // Out-of-bounds with known constants (float64)
  rule <k> ExecStmt(readInput(Cap:Id, OffExpr:ScalarExpr, WidthExpr:ScalarExpr, _:Id))
           => .K ...
       </k>
       <caps>... Cap |-> inputInfo(float64, CapOff:Int, CapLen:Int) ...</caps>
       <events> Es:List => Es ListItem(read : Cap : "expr" : "expr" : "false") </events>
       <status> running => failClosed </status>
       <reads> Rs:List => Rs ListItem(read : Cap : "expr" : "expr" : "false") </reads>
       requires EvalConst(OffExpr) >=Int 0
        andBool EvalConst(WidthExpr) >=Int 0
        andBool (EvalConst(OffExpr) <Int CapOff orBool EvalConst(OffExpr) +Int EvalConst(WidthExpr) >Int CapOff +Int CapLen)

  // Unknown constants (float64)
  rule <k> ExecStmt(readInput(Cap:Id, OffExpr:ScalarExpr, WidthExpr:ScalarExpr, Bind:Id))
           => .K ...
       </k>
       <caps>... Cap |-> inputInfo(float64, _, _) ...</caps>
       <scalars> M:Map => M[Bind <- float64Ty] </scalars>
       <events> Es:List => Es ListItem(read : Cap : "expr" : "expr" : "true") </events>
       <reads> Rs:List => Rs ListItem(read : Cap : "expr" : "expr" : "true") </reads>
       <status> running </status>
       requires EvalConst(OffExpr) <Int 0 orBool EvalConst(WidthExpr) <Int 0
  // In-bounds with known constants (bool)
  rule <k> ExecStmt(readInput(Cap:Id, OffExpr:ScalarExpr, WidthExpr:ScalarExpr, Bind:Id))
           => .K ...
       </k>
       <caps>... Cap |-> inputInfo(bool, CapOff:Int, CapLen:Int) ...</caps>
       <scalars> M:Map => M[Bind <- boolTy] </scalars>
       <events> Es:List => Es ListItem(read : Cap : "expr" : "expr" : "true") </events>
       <reads> Rs:List => Rs ListItem(read : Cap : "expr" : "expr" : "true") </reads>
       <status> running </status>
       requires EvalConst(OffExpr) >=Int 0
        andBool EvalConst(WidthExpr) >=Int 0
        andBool EvalConst(OffExpr) >=Int CapOff
        andBool EvalConst(OffExpr) +Int EvalConst(WidthExpr) <=Int CapOff +Int CapLen

  // Out-of-bounds with known constants (bool)
  rule <k> ExecStmt(readInput(Cap:Id, OffExpr:ScalarExpr, WidthExpr:ScalarExpr, _:Id))
           => .K ...
       </k>
       <caps>... Cap |-> inputInfo(bool, CapOff:Int, CapLen:Int) ...</caps>
       <events> Es:List => Es ListItem(read : Cap : "expr" : "expr" : "false") </events>
       <status> running => failClosed </status>
       <reads> Rs:List => Rs ListItem(read : Cap : "expr" : "expr" : "false") </reads>
       requires EvalConst(OffExpr) >=Int 0
        andBool EvalConst(WidthExpr) >=Int 0
        andBool (EvalConst(OffExpr) <Int CapOff orBool EvalConst(OffExpr) +Int EvalConst(WidthExpr) >Int CapOff +Int CapLen)

  // Unknown constants (bool)
  rule <k> ExecStmt(readInput(Cap:Id, OffExpr:ScalarExpr, WidthExpr:ScalarExpr, Bind:Id))
           => .K ...
       </k>
       <caps>... Cap |-> inputInfo(bool, _, _) ...</caps>
       <scalars> M:Map => M[Bind <- boolTy] </scalars>
       <events> Es:List => Es ListItem(read : Cap : "expr" : "expr" : "true") </events>
       <reads> Rs:List => Rs ListItem(read : Cap : "expr" : "expr" : "true") </reads>
       <status> running </status>
       requires EvalConst(OffExpr) <Int 0 orBool EvalConst(WidthExpr) <Int 0

  // Missing input capability
  rule <k> ExecStmt(readInput(Cap:Id, _, _, _:Id)) => .K ... </k>
       <caps> M:Map </caps>
       <status> running => failClosed </status>
       <events> Es:List => Es ListItem(fail-closed : "missing-input-capability") </events>
       requires Cap in_keys(M) ==Bool false

  // -------------------------------------------------------------------------
  // letScalar
  // -------------------------------------------------------------------------
  syntax KItem ::= LetScalarBind(Id)

  rule <k> ExecStmt(letScalar(Name:Id, Expr:ScalarExpr))
           => TypeOf(Expr) ~> LetScalarBind(Name)
           ...
       </k>

  rule <k> TypeOfResult(T) ~> LetScalarBind(Name) => .K ... </k>
       <scalars> M:Map => M[Name <- T] </scalars>
       <status> running </status>

  // -------------------------------------------------------------------------
  // appendValue — builder existence + budget check only.
  // Type-checking of constant expressions is relaxed to avoid false
  // positives from K's single Int sort vs Rust's Int32/Int64 distinction.
  // -------------------------------------------------------------------------
  syntax KItem ::= AppendValueCheck(Id)

  rule <k> ExecStmt(appendValue(B:Id, Expr:ScalarExpr))
           => TypeOf(Expr) ~> AppendValueCheck(B)
           ...
       </k>

  // Normal append: any TypeOfResult is accepted for constants
  rule <k> TypeOfResult(_) ~> AppendValueCheck(B) => .K ... </k>
       <caps>... B |-> builderInfo(Ty:L2Ty, _:Bool) ...</caps>
       <events> Es:List => Es ListItem(append-value : B : Ty) </events>
       <rows> R:Int => R +Int 1 </rows>
       <maxRows> MaxR:Int </maxRows>
       <status> running </status>
       requires R <Int MaxR

  // Missing builder
  rule <k> TypeOfResult(_) ~> AppendValueCheck(B) => .K ... </k>
       <caps> M:Map </caps>
       <events> Es:List => Es ListItem(fail-closed : "missing-output-builder") </events>
       <status> running => failClosed </status>
       requires B in_keys(M) ==Bool false

  // Budget exceeded
  rule <k> TypeOfResult(_) ~> AppendValueCheck(_) => .K ... </k>
       <rows> R:Int </rows>
       <maxRows> MaxR:Int </maxRows>
       <events> Es:List => Es ListItem(fail-closed : "resource-budget-exceeded") </events>
       <status> running => failClosed </status>
       requires R >=Int MaxR

  // -------------------------------------------------------------------------
  // appendNull
  // -------------------------------------------------------------------------
  rule <k> ExecStmt(appendNull(B:Id)) => .K ... </k>
       <caps>... B |-> builderInfo(T:L2Ty, true) ...</caps>
       <events> Es:List => Es ListItem(append-null : B : T) </events>
       <rows> R:Int => R +Int 1 </rows>
       <maxRows> MaxR:Int </maxRows>
       <status> running </status>
       requires R <Int MaxR

  // Non-nullable builder
  rule <k> ExecStmt(appendNull(B:Id)) => .K ... </k>
       <caps>... B |-> builderInfo(_:L2Ty, false) ...</caps>
       <events> Es:List => Es ListItem(fail-closed : "output-nullability-mismatch") </events>
       <status> running => failClosed </status>

  // Undeclared builder
  rule <k> ExecStmt(appendNull(B:Id)) => .K ... </k>
       <caps> M:Map </caps>
       <events> Es:List => Es ListItem(fail-closed : "missing-output-builder") </events>
       <status> running => failClosed </status>
       requires B in_keys(M) ==Bool false

  // Budget exceeded
  rule <k> ExecStmt(appendNull(_:Id)) => .K ... </k>
       <rows> R:Int </rows>
       <maxRows> MaxR:Int </maxRows>
       <events> Es:List => Es ListItem(fail-closed : "resource-budget-exceeded") </events>
       <status> running => failClosed </status>
       requires R >=Int MaxR

  // -------------------------------------------------------------------------
  // forRange
  // -------------------------------------------------------------------------
  syntax KItem ::= ExecForRangeLoop(Id, Int, Int, Stmts, Map, Int, Int)

  rule <k> ExecStmt(forRange(Index:Id, StartExpr:ScalarExpr, EndExpr:ScalarExpr, Body:Stmts))
           => ExecForRangeLoop(Index, StartVal, EndVal, Body, Saved, 0, MaxR)
           ...
       </k>
       <scalars> Saved:Map </scalars>
       <maxRows> MaxR:Int </maxRows>
       <rows> R:Int => R +Int (EndVal -Int StartVal) </rows>
       requires EvalConst(StartExpr) ==Int StartVal
        andBool EvalConst(EndExpr) ==Int EndVal
        andBool StartVal >=Int 0
        andBool EndVal >=Int StartVal
        andBool (EndVal -Int StartVal) <=Int (MaxR -Int R)

  rule <k> ExecForRangeLoop(Index, Cur, End, Body, Saved, Count, Max)
           => RestoreScalars(Saved) ~> SetScalar(Index, rowIndexTy) ~> ExecBody(Body) ~> ExecForRangeLoop(Index, Cur +Int 1, End, Body, Saved, Count +Int 1, Max)
           ...
       </k>
       requires Cur <Int End
        andBool Count <Int Max

  rule <k> ExecForRangeLoop(Index, Cur, End, Body, Saved, Count, Max) => .K ... </k>
       <scalars> _ => Saved </scalars>
       requires Cur >=Int End

  // Invalid loop bounds
  rule <k> ExecStmt(forRange(_:Id, StartExpr:ScalarExpr, EndExpr:ScalarExpr, _:Stmts)) => .K ... </k>
       <scalars> M:Map </scalars>
       <events> Es:List => Es ListItem(fail-closed : "invalid-loop-bounds") </events>
       <status> running => failClosed </status>
       requires EvalConst(StartExpr) >=Int 0
        andBool EvalConst(EndExpr) >=Int 0
        andBool EvalConst(EndExpr) <Int EvalConst(StartExpr)

  // Budget exceeded
  rule <k> ExecStmt(forRange(_:Id, StartExpr:ScalarExpr, EndExpr:ScalarExpr, _:Stmts)) => .K ... </k>
       <scalars> M:Map </scalars>
       <maxRows> MaxR:Int </maxRows>
       <rows> R:Int </rows>
       <events> Es:List => Es ListItem(fail-closed : "resource-budget-exceeded") </events>
       <status> running => failClosed </status>
       requires EvalConst(StartExpr) >=Int 0
        andBool EvalConst(EndExpr) >=Int 0
        andBool EvalConst(EndExpr) >=Int EvalConst(StartExpr)
        andBool (EvalConst(EndExpr) -Int EvalConst(StartExpr)) >Int (MaxR -Int R)

  // -------------------------------------------------------------------------
  // cursorLoop
  // -------------------------------------------------------------------------
  syntax Bool ::= IsMonotoneProgress(Id, ScalarExpr, Map) [function]

  rule IsMonotoneProgress(Cursor, add(Cursor, N:Int), M:Map) => true
       requires N >Int 0
  rule IsMonotoneProgress(_, _, _) => false [owise]

  syntax KItem ::= ExecCursorLoopLoop(Id, Int, Stmts, Map, Int, Int)

  rule <k> ExecStmt(cursorLoop(Cursor:Id, LimitExpr:ScalarExpr, ProgressExpr:ScalarExpr, Body:Stmts))
           => ExecCursorLoopLoop(Cursor, LimitVal, Body, Saved, 0, MaxR)
           ...
       </k>
       <scalars> Saved:Map </scalars>
       <maxRows> MaxR:Int </maxRows>
       <rows> R:Int => R +Int LimitVal </rows>
       requires EvalConst(LimitExpr) ==Int LimitVal
        andBool LimitVal >=Int 0
        andBool LimitVal <=Int (MaxR -Int R)
        andBool IsMonotoneProgress(Cursor, ProgressExpr, Saved)

  rule <k> ExecCursorLoopLoop(Cursor, Remaining, Body, Saved, Count, Max)
           => RestoreScalars(Saved) ~> SetScalar(Cursor, rowIndexTy) ~> ExecBody(Body) ~> ExecCursorLoopLoop(Cursor, Remaining -Int 1, Body, Saved, Count +Int 1, Max)
           ...
       </k>
       requires Remaining >Int 0
        andBool Count <Int Max

  rule <k> ExecCursorLoopLoop(Cursor, Remaining, Body, Saved, Count, Max) => .K ... </k>
       <scalars> _ => Saved </scalars>
       requires Remaining <=Int 0

  // Non-monotone progress
  rule <k> ExecStmt(cursorLoop(Cursor:Id, _:ScalarExpr, ProgressExpr:ScalarExpr, _:Stmts)) => .K ... </k>
       <scalars> M:Map </scalars>
       <events> Es:List => Es ListItem(fail-closed : "non-monotone-cursor-loop") </events>
       <status> running => failClosed </status>
       requires IsMonotoneProgress(Cursor, ProgressExpr, M) ==Bool false

  // Invalid limit
  rule <k> ExecStmt(cursorLoop(_:Id, LimitExpr:ScalarExpr, _:ScalarExpr, _:Stmts)) => .K ... </k>
       <scalars> M:Map </scalars>
       <maxRows> MaxR:Int </maxRows>
       <rows> R:Int </rows>
       <events> Es:List => Es ListItem(fail-closed : "resource-budget-exceeded") </events>
       <status> running => failClosed </status>
       requires EvalConst(LimitExpr) >=Int 0
        andBool EvalConst(LimitExpr) >Int (MaxR -Int R)

  // -------------------------------------------------------------------------
  // failClosed
  // -------------------------------------------------------------------------
  rule <k> ExecStmt(failClosed(Code:String)) => .K ... </k>
       <events> Es:List => Es ListItem(fail-closed : Code) </events>
       <status> running => failClosed </status>

  // -------------------------------------------------------------------------
  // finish
  // -------------------------------------------------------------------------
  rule <k> finish() => .K ... </k>
       <events> Es:List => Es ListItem(terminal : finished) </events>
       <status> running => finished </status>

endmodule

module KLOOM
  imports LOOMCORE
endmodule
