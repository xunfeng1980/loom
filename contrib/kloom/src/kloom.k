/*
  kloom — K Framework executable semantics for Loom L2Core (pure-append subset, v0)

  Trust model: spec-oracle, offline/CI only, outside production TCB.
*/

module LOOMCORE-SYNTAX
  imports INT
  imports BOOL

  syntax Id ::= r"[a-zA-Z][a-zA-Z0-9_]*" [token]

  syntax L2Ty ::= "int32"
                | "int64"
                | "float32"
                | "float64"
                | "bool"

  syntax ScalarValue ::= Int
  syntax ScalarExpr  ::= ScalarValue

  syntax Capability ::= "input" Id ":" L2Ty
                      | "builder" Id ":" L2Ty
                      | "builder" Id ":" L2Ty "nullable"

  syntax Capabilities ::= List{Capability, ","}

  syntax Stmt ::= "appendValue" "(" Id "," ScalarExpr ")"
                | "appendNull"  "(" Id ")"

  syntax Stmts ::= List{Stmt, ";"}

  syntax Program ::= "program" Capabilities "body" Stmts "maxRows" Int

  syntax TraceEvent ::= "append-value" ":" Id ":" L2Ty
                      | "append-null"  ":" Id ":" L2Ty
                      | "terminal"     ":" "finished"
endmodule


module LOOMCORE
  imports LOOMCORE-SYNTAX
  imports INT
  imports BOOL
  imports LIST
  imports MAP

  // -------------------------------------------------------------------------
  // Internal capability representation
  // -------------------------------------------------------------------------
  syntax CapInfo ::= inputInfo(L2Ty)
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
  // initCaps — surface syntax → internal representation
  // -------------------------------------------------------------------------
  syntax KItem ::= initCaps(Capabilities)

  rule initCaps(.Capabilities) => .K

  rule <k> initCaps(input N:Id : T:L2Ty , Cs:Capabilities)
           => initCaps(Cs) ...
       </k>
       <caps> M:Map => M[N <- inputInfo(T)] </caps>

  rule <k> initCaps(builder N:Id : T:L2Ty , Cs:Capabilities)
           => initCaps(Cs) ...
       </k>
       <caps> M:Map => M[N <- builderInfo(T, false)] </caps>

  rule <k> initCaps(builder N:Id : T:L2Ty nullable , Cs:Capabilities)
           => initCaps(Cs) ...
       </k>
       <caps> M:Map => M[N <- builderInfo(T, true)] </caps>

  // -------------------------------------------------------------------------
  // Program initialization
  // -------------------------------------------------------------------------
  rule <k> (program Cs:Capabilities body Ss:Stmts maxRows MR:Int)
           => initCaps(Cs) ~> execBody(Ss) ~> finish()
           ...
       </k>
       <maxRows> _ => MR </maxRows>

  // -------------------------------------------------------------------------
  // Statement dispatch
  // -------------------------------------------------------------------------
  syntax KItem ::= execBody(Stmts)
                 | execStmt(Stmt)
                 | finish() [symbol(finish)]

  rule execBody(.Stmts) => .K
  rule execBody(S:Stmt ; Ss:Stmts) => execStmt(S) ~> execBody(Ss)

  // -------------------------------------------------------------------------
  // appendValue
  // -------------------------------------------------------------------------
  rule <k> execStmt(appendValue(B:Id,_V:ScalarExpr)) => .K ... </k>
       <caps>... B |-> builderInfo(T:L2Ty, _:Bool) ...</caps>
       <events> Es:List => Es ListItem(append-value : B : T) </events>
       <rows> R:Int => R +Int 1 </rows>
       <maxRows> MaxR:Int </maxRows>
       <status> running </status>
    requires R <Int MaxR

  // Undeclared builder → failClosed
  rule <k> execStmt(appendValue(B:Id, _:ScalarExpr)) => .K ... </k>
       <caps> M:Map </caps>
       <status> running => failClosed </status>
    requires notBool(B in_keys(M))

  // Budget exceeded → failClosed
  rule <k> execStmt(appendValue(_:Id, _:ScalarExpr)) => .K ... </k>
       <rows> R:Int </rows>
       <maxRows> MaxR:Int </maxRows>
       <status> running => failClosed </status>
    requires R >=Int MaxR

  // -------------------------------------------------------------------------
  // appendNull
  // -------------------------------------------------------------------------
  rule <k> execStmt(appendNull(B:Id)) => .K ... </k>
       <caps>... B |-> builderInfo(T:L2Ty, true) ...</caps>
       <events> Es:List => Es ListItem(append-null : B : T) </events>
       <rows> R:Int => R +Int 1 </rows>
       <maxRows> MaxR:Int </maxRows>
       <status> running </status>
    requires R <Int MaxR

  // Non-nullable builder → failClosed
  rule <k> execStmt(appendNull(B:Id)) => .K ... </k>
       <caps>... B |-> builderInfo(_:L2Ty, false) ...</caps>
       <status> running => failClosed </status>

  // Undeclared builder → failClosed
  rule <k> execStmt(appendNull(B:Id)) => .K ... </k>
       <caps> M:Map </caps>
       <status> running => failClosed </status>
    requires notBool(B in_keys(M))

  // Budget exceeded → failClosed
  rule <k> execStmt(appendNull(_:Id)) => .K ... </k>
       <rows> R:Int </rows>
       <maxRows> MaxR:Int </maxRows>
       <status> running => failClosed </status>
    requires R >=Int MaxR

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
