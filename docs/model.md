# Model reference

`iwr analyze --pretty` / `GET /api/project` return a `Project`. Ids are indexes into the arrays. Lines/cols are 1-based. Enums serialize as `snake_case`.

```
Project { name, root, files[], modules[], items[], calls[], type_rels[], main?, external_crates[], diagnostics[] }
SourceFile { id, path, content }
Span { file, line_start, col_start, line_end, col_end }
Module { id, name, path, file?, parent?, children[], items[], uses[], span?, doc? }
Item { id, kind, name, path, module, vis, span, signature, doc?, attrs[], extra }
  kind: function | method | struct | enum | trait | type_alias | const | static | module | impl
  vis:  private | crate | public | restricted
  extra (tagged by "kind"):
    fn         FnInfo
    struct     { fields[Field], derives[], is_tuple, is_unit }
    enum       { variants[{ name, fields[] }], derives[] }
    trait      { supertraits[], supertrait_ids[], methods[] }
    impl       { target, target_id?, trait_name?, trait_id?, methods[] }
    type_alias { target } | const { ty } | module
Field { name, ty, vis, refs[] }                  refs = project types used in ty
FnInfo { is_async, is_unsafe, is_const, self_kind, impl_id?, owner_type?, owner_type_name?, trait_id?, trait_name?,
         params[{name, ty}], ret?, returns_result, returns_option, result_ok?, result_err?,
         question_marks, unwraps, expects, panics, awaits, loops, branches, is_recursive,
         cfg?, blocks?, body_lines, is_test, is_main }
CallEdge { from, to?, callee, kind, span, in_loop, in_branch, propagated, unwrapped, is_await, is_recursive }
  kind: call | method_call | macro | construct        to = None for external / macros
TypeRel { from, to, kind, label? }   kind: contains | implements | supertrait | uses_in_signature | aliases
```

## Cfg (per function)

```
Cfg { nodes[CfgNode], edges[CfgEdge] }          node 0 = entry, node 1 = exit
CfgNode { id, kind, label, detail, span, calls[], depth }
  kind: entry | exit | block | if | match | loop | call | return | jump | propagate | panic | await
CfgEdge { from, to, label?, kind }
  kind: next | true | false | arm | loop_body | loop_back | break | continue | error | return
```

Consecutive plain statements merge into one `block` node (≤ 4 lines). `propagate`/`panic` nodes have an extra `error` edge to the exit.

## Block (per function, Code view)

```
Block { id, kind, label, detail, span, children[], defines[], uses[], calls[] }
  kind: fn | let | stmt | call | macro | if | else_if | else | match | arm | loop
      | return | break | continue | propagate | panic | await | unsafe | closure
```

`defines` = names bound by the block (let pattern, loop pattern, arm pattern, closure args, fn params on the root). `uses` = in-scope locals read by the block's own head (not its children). `deepest_at(line)` finds the innermost block for a line.

## Views (`iwr_core::views`)

```
Graph { mode?, nodes[VNode], edges[VEdge], width, height, note? }
VNode { id, label, sublabel, kind, item?, cfg_node?, span?, x, y, w, h, expandable, expanded, badges[], depth, parent?, container }
VEdge { id, from, to, kind, label?, points[(x,y)], backward }
ViewOptions { root?, module?, expanded{}, collapsed{}, depth, show_external, show_macros, show_tests, show_constructs, max_nodes }
```

Node ids are stable per view (`i<item>`, `i<item>/i<callee>` in trees, `c<n>` for cfg nodes, `m<module>`, `t<type>`), so `expanded` sets survive re-analysis.

## Guide

```
GuideStep { title, text (markdown subset), mode, focus_item?, focus_cfg?, highlight_items[], span?, stack[], kind }
  kind: overview | enter | statement | branch | loop | call | error | return | leave
```
