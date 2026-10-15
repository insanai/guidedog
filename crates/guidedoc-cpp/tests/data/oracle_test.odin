package sphinx

import "core:fmt"
import "core:os"
import "core:strings"
import "core:testing"
import gd "../../core"

// Reads corpus.txt (kind TAB text per line) and writes oracle.tsv: what the Odin C++
// machinery gives for each line.

@(private = "file")
new_parser :: proc(def: string) -> Cpp_Parser {
    nodes := make([]Cpp_Node, 16 * len(def) + 256)
    s := new(gd.Scratch)
    s.bytes = make([]u8, 1 << 20)
    return Cpp_Parser{def = def, nodes = nodes, scratch = s, max_depth = 256,
        max_steps = CPP_STEPS_PER_BYTE * len(def) + 4096}
}

@(private = "file")
str :: proc(q: ^Cpp_Parser, id: i32, display := false) -> string {
    if id == NIL do return ""
    b := gd.Buffer{bytes = make([]u8, 64 * len(q.def) + 1024)}
    cpp_write(q, &b, id, display)
    return strings.clone(gd.buffer_string(&b))
}

@(private = "file")
ids_ctx :: proc(q: ^Cpp_Parser) -> Cpp_Ids {
    return Cpp_Ids{p = q, arena = {bytes = make([]u8, 64 * len(q.def) + 4096)},
        out = {bytes = make([]u8, 64 * len(q.def) + 4096)}}
}

@(private = "file")
enc_elements :: proc(sb: ^strings.Builder, elements: []Cpp_Element) {
    for e, i in elements {
        if i > 0 do strings.write_string(sb, ";")
        fmt.sbprintf(sb, "%s|%s|%s|%s", e.ident, e.args, e.params, e.disp)
        for v in 1 ..= 4 {
            strings.write_string(sb, "|")
            strings.write_string(sb, e.ids[v] if e.has[v] else "-")
        }
    }
}

@(private = "file")
stop_name :: proc(q: ^Cpp_Parser) -> string {
    switch q.stopped {
    case .None: return ""
    case .Depth: return "depth"
    case .Stack: return "stack"
    case .Work: return "work"
    }
    return ""
}

@(private = "file")
scope_elements :: proc(scope: string) -> []Cpp_Element {
    if scope == "" do return nil
    q := new(Cpp_Parser)
    q^ = new_parser(scope)
    ns, ok := cpp_parse_namespace(q)
    if !ok do return nil
    name := cpp_at(q, ns).a
    lists := NIL
    if prefix := cpp_at(q, ns).b; prefix != NIL do lists = cpp_at(q, prefix).a
    x := new(Cpp_Ids)
    x^ = ids_ctx(q)
    out := make([]Cpp_Element, 32)
    n := cpp_name_elements(q, x, name, lists, out)
    return out[:n]
}

@(private = "file")
declaration_line :: proc(sb: ^strings.Builder, directive, scope, def: string) {
    q := new_parser(def)
    object_type := cpp_object_type(directive)
    decl, parsed := cpp_parse_declaration(&q, object_type, directive)
    if parsed {
        cpp_skip_ws(&q)
        parsed = cpp_eof(&q)
    }
    if q.stopped != .None {
        fmt.sbprintf(sb, "limit:%s", stop_name(&q))
        return
    }
    name := cpp_declared_name(&q, decl) if parsed else NIL
    if !parsed || name == NIL {
        strings.write_string(sb, "invalid")
        return
    }
    scope_els := scope_elements(scope)
    elements := make([]Cpp_Element, len(scope_els) + cpp_count(&q, cpp_at(&q, name).a))
    copy(elements, scope_els)
    x := ids_ctx(&q)
    lists, _ := cpp_template_lists(&q, decl)
    cpp_name_elements(&q, &x, name, lists, elements[len(scope_els):])
    x.elements = elements
    ids: [5]string
    has: [5]bool
    for v in 1 ..= 4 {
        x.version = v
        start := x.out.used
        has[v] = cpp_declaration_id(&x, &x.out, decl) && !x.out.overflow
        ids[v] = string(x.out.bytes[start:x.out.used])
    }
    if !has[4] {
        strings.write_string(sb, "noid")
        return
    }
    strings.write_string(sb, "ok")
    for v in 1 ..= 4 {
        strings.write_string(sb, "\t")
        strings.write_string(sb, ids[v] if has[v] else "-")
    }
    full, disp: strings.Builder
    for e, i in elements {
        if i > 0 { strings.write_string(&full, "::"); strings.write_string(&disp, "::") }
        strings.write_string(&full, e.ident)
        strings.write_string(&full, e.args)
        strings.write_string(&disp, e.disp)
    }
    fmt.sbprintf(sb, "\t%s\t%s\t", strings.to_string(full), strings.to_string(disp))
    // tparams
    if elements[len(elements) - 1].params != "" {
        first, count := cpp_template_lists(&q, decl)
        last := cpp_list_at(&q, first, count - 1)
        if last != NIL {
            l := cpp_at(&q, last)
            params := l.a if l.kind == .Template_Params else l.b
            k := 0
            for it := params; it != NIL; it = cpp_at(&q, it).next {
                ident := cpp_param_identifier(&q, it)
                if ident == NIL do continue
                if k > 0 do strings.write_string(sb, ",")
                strings.write_string(sb, cpp_at(&q, ident).text)
                k += 1
            }
        }
    }
    strings.write_string(sb, "\t")
    n := cpp_at(&q, decl)
    if n.text == "function" {
        pq := NIL
        for i := cpp_at(&q, n.b).b; i != NIL; {
            m := cpp_at(&q, i)
            #partial switch m.kind {
            case .D_Name: pq = m.c; i = NIL
            case .D_Ptr, .D_Ref, .D_Pack, .D_Paren: i = m.a
            case .D_Mem_Ptr: i = m.b
            case: i = NIL
            }
        }
        if pq != NIL {
            k := 0
            for it := cpp_at(&q, pq).a; it != NIL; it = cpp_at(&q, it).next {
                arg := cpp_at(&q, it).a
                if arg == NIL do continue
                nn := cpp_declared_name(&q, arg)
                if nn == NIL do continue
                s := str(&q, nn)
                if s == "" do continue
                if k > 0 do strings.write_string(sb, ",")
                strings.write_string(sb, s)
                k += 1
            }
        }
    }
    fmt.sbprintf(sb, "\t%s\t%s\t", str(&q, decl), str(&q, decl, true))
    enc_elements(sb, elements)
    fmt.sbprintf(sb, "\t%v", q.fallback)
}

@(private = "file")
xref_line :: proc(sb: ^strings.Builder, target: string, parens: bool) {
    def := strings.concatenate({trim_space_cpp(target), "()" if parens else ""})
    q := new_parser(def)
    res, shorthand, parsed := cpp_parse_xref(&q)
    if q.stopped != .None {
        fmt.sbprintf(sb, "limit:%s", stop_name(&q))
        return
    }
    if !parsed {
        strings.write_string(sb, "invalid")
        return
    }
    n := cpp_at(&q, res)
    name := n.a if shorthand else cpp_declared_name(&q, res)
    prefix := n.b if shorthand else n.a
    fmt.sbprintf(sb, "ok\t%v\t", shorthand)
    for e := cpp_at(&q, name).a; e != NIL; e = cpp_at(&q, e).next {
        if e != cpp_at(&q, name).a do strings.write_string(sb, ";")
        fmt.sbprintf(sb, "%s|%s", str(&q, cpp_at(&q, e).a), str(&q, cpp_at(&q, e).b))
    }
    fmt.sbprintf(sb, "\t%v\t", .Rooted in cpp_at(&q, name).flags)
    lists := NIL
    if prefix != NIL && .None_List not_in cpp_at(&q, prefix).flags do lists = cpp_at(&q, prefix).a
    for t := lists; t != NIL; t = cpp_at(&q, t).next {
        if t != lists do strings.write_string(sb, ";")
        strings.write_string(sb, str(&q, t))
    }
    strings.write_string(sb, "\t")
    if shorthand do return
    x := ids_ctx(&q)
    x.version = 4
    x.placeholder = true
    if !cpp_declaration_id(&x, &x.out, res) || x.out.overflow do return
    q2, _ := strings.replace_all(gd.buffer_string(&x.out), "\x1d", "$")
    strings.write_string(sb, q2)
    x.placeholder = false
    strings.write_string(sb, "\t")
    if count := cpp_count(&q, cpp_at(&q, name).a); count > 0 {
        elements := make([]Cpp_Element, count)
        cpp_name_elements(&q, &x, name, lists, elements)
        if elements[count - 1].has[4] do strings.write_string(sb, elements[count - 1].ids[4])
    }
}

@(private = "file")
ns_line :: proc(sb: ^strings.Builder, base, arg: string) {
    def := trim_space_cpp(arg)
    if def == "NULL" || def == "0" || def == "nullptr" {
        strings.write_string(sb, "root")
        return
    }
    q := new_parser(def)
    ns, parsed := cpp_parse_namespace(&q)
    if q.stopped != .None {
        fmt.sbprintf(sb, "limit:%s", stop_name(&q))
        return
    }
    if parsed {
        cpp_skip_ws(&q)
        parsed = cpp_eof(&q)
    }
    if !parsed {
        strings.write_string(sb, "invalid")
        return
    }
    name := cpp_at(&q, ns).a
    lists := NIL
    if prefix := cpp_at(&q, ns).b; prefix != NIL do lists = cpp_at(&q, prefix).a
    base_els := scope_elements(base)
    elements := make([]Cpp_Element, len(base_els) + cpp_count(&q, cpp_at(&q, name).a))
    copy(elements, base_els)
    x := ids_ctx(&q)
    cpp_name_elements(&q, &x, name, lists, elements[len(base_els):])
    strings.write_string(sb, "ok\t")
    enc_elements(sb, elements)
}

@(private = "file")
expr_line :: proc(sb: ^strings.Builder, text: string) {
    def := trim_space_cpp(text)
    if def == "" {
        strings.write_string(sb, "invalid")
        return
    }
    q := new_parser(def)
    ast, parsed := cpp_parse_expression_or_type(&q)
    if q.stopped != .None {
        fmt.sbprintf(sb, "limit:%s", stop_name(&q))
        return
    }
    if !parsed || q.fallback {
        strings.write_string(sb, "invalid")
        return
    }
    fmt.sbprintf(sb, "ok\t%s\t%s\t%v", str(&q, ast), str(&q, ast, true), cpp_at(&q, ast).kind)
    for v in 2 ..= 4 {
        x := ids_ctx(&q)
        x.version = v
        if cpp_id(&x, &x.out, ast) && !x.out.overflow {
            fmt.sbprintf(sb, "\t%s", gd.buffer_string(&x.out))
        } else {
            strings.write_string(sb, "\t-")
        }
    }
}

@(test)
oracle :: proc(t: ^testing.T) {
    dir := #directory
    data, err := os.read_entire_file(strings.concatenate({dir, "/../../../corpus.txt"}), context.allocator)
    if err != nil do return
    sb: strings.Builder
    for line in strings.split_lines(string(data)) {
        if line == "" || line[0] == '#' do continue
        parts := strings.split_n(line, "\t", 2)
        if len(parts) < 2 do continue
        kind, text := parts[0], parts[1]
        strings.write_string(&sb, line)
        strings.write_string(&sb, "\t=>\t")
        switch {
        case kind == "xref": xref_line(&sb, text, false)
        case kind == "xref()": xref_line(&sb, text, true)
        case kind == "expr": expr_line(&sb, text)
        case strings.has_prefix(kind, "ns"):
            base := kind[3:] if len(kind) > 3 else ""
            ns_line(&sb, base, text)
        case:
            directive, scope := kind, ""
            if k := strings.index(kind, "@"); k >= 0 do directive, scope = kind[:k], kind[k + 1:]
            declaration_line(&sb, directive, scope, text)
        }
        strings.write_string(&sb, "\n")
    }
    _ = os.write_entire_file(strings.concatenate({dir, "/../../../oracle.tsv"}), sb.buf[:])
}
