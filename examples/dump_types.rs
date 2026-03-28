
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let (hdr, body) = xt_parser::header::split_header(&text).unwrap();
    let (_, _, body_stripped) = xt_parser::schema::parse_tline(body).unwrap();
    let mut input = body_stripped.as_str();
    let preamble = xt_parser::schema::parse_schema_preamble(&mut input).unwrap();
    let entities = xt_parser::entity::parse_entities(&mut input, preamble.partition_count).unwrap();
    let n = entities.len();
    for (i, e) in entities.iter().enumerate().skip(n.saturating_sub(5)) {
        eprintln!("[{:3}] type={:3} idx={:4} fields={} var_f64={} var_i16={} var_ptr={} var_char={}",
            i, e.type_id, e.index, e.fields.len(),
            e.var_f64.len(), e.var_i16.len(), e.var_ptr.len(), e.var_char.len());
    }
}
