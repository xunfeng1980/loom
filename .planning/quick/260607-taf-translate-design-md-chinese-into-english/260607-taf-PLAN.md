---
phase: quick
plan: 260607-taf
type: execute
wave: 1
depends_on: []
files_modified:
  - README.md
  - README-zh.md
autonomous: true
requirements: [TRANSLATE-01]

must_haves:
  truths:
    - "README.md at repo root is a faithful, professional English translation of design.md"
    - "README-zh.md at repo root is the Chinese version, structurally 1:1 with README.md"
    - "All 15 sections (0-14), headings, tables, code/ASCII blocks, blockquotes, and emphasis are preserved in both files"
    - "design.md is left untouched as the original source"
  artifacts:
    - path: "README.md"
      provides: "English translation of the Loom design document"
      contains: "Loom"
    - path: "README-zh.md"
      provides: "Chinese design document, mirror of README.md structure"
      contains: "Loom"
  key_links:
    - from: "README.md"
      to: "design.md"
      via: "section-by-section translation (0-14)"
      pattern: "## 0\\."
---

<objective>
Translate the Chinese design document `design.md` ("分发型解码 IR 设计方案 / 工作代号:Loom", 214 lines) into a faithful, professional English `README.md` at the repo root, and create a Chinese `README-zh.md` at the repo root whose structure mirrors `README.md` 1:1 so the two are translations of each other.

Purpose: Make the core Loom design accessible in English while keeping a maintained Chinese version structurally aligned with it.
Output: `/Users/macintoshhd/loom-demo/README.md` (English) and `/Users/macintoshhd/loom-demo/README-zh.md` (Chinese). `design.md` is left in place untouched as the origin document.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@design.md
@CLAUDE.md

# This is a translation task. CLAUDE.md carries the canonical product framing
# (English) and the proper-noun list (Loom, Vortex, Arrow, MLIR, LLVM, etc.)
# to keep terminology consistent with the rest of the codebase docs.
</context>

<decisions>
- design.md stays in place, untouched. It is the origin/source document. README-zh.md becomes the maintained Chinese doc going forward; design.md is the historical source and is NOT deleted or modified. (Per task requirement 4.)
- README-zh.md content equals design.md content, reflowed only as needed to stay structurally identical to README.md (same section order, same tables, same code blocks). No content is added, dropped, or editorialized in either direction.
- Language-switch link is included as a nice-to-have: a single line at the very top of each file linking to the other (English | 中文), since it is low-cost and improves navigation. (Per task requirement 5 — optional, decided IN.)
</decisions>

<terminology_glossary>
Translate these consistently in README.md (English). Keep proper nouns AS-IS in both files.

| Chinese | English |
|---------|---------|
| 全函数 | total function |
| 非图灵完备 | non-Turing-complete |
| 声明式布局层 | declarative layout layer |
| 全函数内核层 | total-function kernel layer |
| 递减度量 | decreasing measure / ranking function |
| 良构 | well-formed |
| capability 句柄 | capability handle |
| 分发型解码 IR | distribution-oriented decoder IR |
| 目标中立 | target-neutral |
| 信任边界 | trust boundary |
| 沙箱 | sandbox |
| 攻击面 | attack surface |
| 内容哈希 URI | content-hash URI |
| feature flags | feature flags (keep) |
| 谓词 | predicate |
| 列裁剪 | column projection / pruning |
| 偏移驱动 | offset-driven |
| 重复 | repetition |
| 字典 | dictionary |

Proper nouns kept verbatim (both files): Loom, Vortex, Arrow, MLIR, LLVM, Substrait, FastLanes, eBPF, Wasm, AnyBlox, LingoDB, PNaCl, Kaitai Struct, DFDL, ROOT, FSST, ALP, Parquet, SIMD, SVE, Memory64, mmap, CSE, JIT, TCB, MPP.
</terminology_glossary>

<tasks>

<task type="auto">
  <name>Task 1: Write README.md — faithful English translation of design.md</name>
  <files>README.md</files>
  <action>
    Create /Users/macintoshhd/loom-demo/README.md as a faithful, professional English
    translation of design.md (per TRANSLATE-01). Translate section-by-section, sections 0
    through 14, preserving EXACTLY: section numbering 0-14 and all heading levels (the H1
    title, ## section headings, and bold sub-headers like 5.1/5.2); every table including
    the §2 three-IR-axes table and especially the §12 comparison matrix — preserve the
    check/cross/triangle marks verbatim and translate the column headers (distribution-portable,
    untrusted sandbox, total function / provably terminating, native full-speed, target-neutral /
    version-stable, mandatory Arrow output); the fenced code/diagram blocks — the ASCII relay
    pipeline in §2 and the ABI block in §9 (schema / decode_batch / statistics signatures) — kept
    inside triple-backtick fences, translating only inline Chinese comments and leaving identifiers
    as-is; all blockquotes (the lines beginning with > in §6, §7, §13) and all bold/emphasis.
    Apply the terminology glossary above for consistency. Keep ALL proper nouns verbatim. Do NOT
    add, drop, summarize, or editorialize content — this is a translation, not a rewrite. Add ONE
    line at the very top, above the H1 title: a language-switch link of the form **English** then
    a separator then a markdown link to README-zh.md labelled 中文.
  </action>
  <verify>
    <automated>test -f /Users/macintoshhd/loom-demo/README.md && grep -q '^## 14\.' /Users/macintoshhd/loom-demo/README.md && grep -q 'README-zh.md' /Users/macintoshhd/loom-demo/README.md && grep -F -q '✓' /Users/macintoshhd/loom-demo/README.md && grep -F -q '△' /Users/macintoshhd/loom-demo/README.md && echo PASS</automated>
  </verify>
  <done>README.md exists at repo root, contains all sections 0-14 in English, the §12 comparison matrix retains its check/cross/triangle marks, the §2 ASCII pipeline and §9 ABI blocks are present as fenced code, the language-switch link is at the top, and no content was added or dropped relative to design.md.</done>
</task>

<task type="auto">
  <name>Task 2: Write README-zh.md — Chinese version mirroring README.md structure</name>
  <files>README-zh.md</files>
  <action>
    Create /Users/macintoshhd/loom-demo/README-zh.md as the Chinese version, content consistent
    and 1:1 with README.md (per TRANSLATE-01). Because design.md is already the Chinese source,
    README-zh.md carries design.md's Chinese text, reflowed ONLY where needed so its structure is
    identical to README.md: same section order (0-14), same heading levels, same tables (the §12
    matrix with the same check/cross/triangle marks and identical column count), same fenced code
    blocks (§2 ASCII pipeline, §9 ABI). Do NOT change wording, add, or drop content versus
    design.md — it is the same Chinese content, just confirmed structurally aligned to README.md.
    Add ONE line at the very top, above the H1 title: a language-switch link of the form a markdown
    link to README.md labelled English, then a separator, then **中文**. Leave design.md itself
    untouched (do not edit or delete it).
  </action>
  <verify>
    <automated>test -f /Users/macintoshhd/loom-demo/README-zh.md && grep -q '^## 14\.' /Users/macintoshhd/loom-demo/README-zh.md && grep -q 'README.md' /Users/macintoshhd/loom-demo/README-zh.md && grep -F -q '✓' /Users/macintoshhd/loom-demo/README-zh.md && test -f /Users/macintoshhd/loom-demo/design.md && echo PASS</automated>
  </verify>
  <done>README-zh.md exists at repo root with all sections 0-14 in Chinese, structurally identical to README.md (same sections, tables, code blocks), the §12 matrix marks preserved, the language-switch link at the top, and design.md remains present and unmodified.</done>
</task>

</tasks>

<verification>
- Both README.md and README-zh.md exist at /Users/macintoshhd/loom-demo/ root.
- Section count matches design.md: 15 sections numbered 0 through 14 in each file.
- The §12 comparison matrix row/column count and ✓/✗/△ marks are identical across design.md, README.md, and README-zh.md.
- design.md is byte-identical to its pre-task state (untouched).
- Cross-links resolve: README.md links to README-zh.md and vice versa.
</verification>

<success_criteria>
- README.md is a complete, faithful, professional English translation of design.md with all structure (sections 0-14, headings, tables, fenced code blocks, blockquotes, emphasis) preserved.
- README-zh.md is the Chinese counterpart, structurally 1:1 with README.md.
- Terminology is consistent per the glossary; proper nouns are verbatim.
- No content added, dropped, or editorialized in either file.
- design.md left untouched.
</success_criteria>

<output>
Create `.planning/quick/260607-taf-translate-design-md-chinese-into-english/260607-taf-SUMMARY.md` when done.
</output>
