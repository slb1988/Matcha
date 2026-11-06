use bc_formats::{
    bcff::{parse_bcfileformats, write_bcfileformats},
    model::{DateOrder, HeaderMode, RegionalSettings, TableType},
};
#[test]
fn table_type_regional_roundtrip_and_defaults() {
    let xml = include_str!("../data/bcfileformats.xml");
    let formats = parse_bcfileformats(xml).unwrap();
    assert_eq!(formats.len(), 25);
    let mut csv = formats.iter().find(|f| f.name == "CSV").unwrap().clone();
    let excel = formats.iter().find(|f| f.name == "MSEXCEL").unwrap();
    assert_eq!(csv.table.as_ref().unwrap().delimiters, vec![',']);
    assert_eq!(excel.table.as_ref().unwrap().delimiters, vec!['\t']);
    let unchanged = write_bcfileformats(&formats).unwrap();
    for name in [
        "TableType",
        "FixedWidths",
        "FirstRow",
        "DlmConsecutive",
        "DlmTrimWhitespace",
        "DlmDetect",
        "UseSystemRegional",
        "DecimalSeparator",
        "ThousandsSeparator",
        "DateOrder",
        "DateSeparator",
    ] {
        assert!(!unchanged.contains(&format!("<{name}")));
    }
    let options = csv.table.as_mut().unwrap();
    options.table_type = TableType::Fixed;
    options.column_widths = vec![5, 10, 3];
    options.header = HeaderMode::Names;
    options.delimiters = vec![';', '\t', '界'];
    options.qualifier = Some('\'');
    options.consecutive_delimiters = true;
    options.trim_surrounding_whitespace = true;
    options.detect_delimiter = true;
    options.regional = RegionalSettings {
        use_system: false,
        decimal: ',',
        thousands: Some('.'),
        date_order: DateOrder::Dmy,
        date_separator: '/',
    };
    let changed = write_bcfileformats(&[csv.clone()]).unwrap();
    assert_eq!(parse_bcfileformats(&changed).unwrap(), vec![csv.clone()]);
    csv.source_xml = None;
    assert_eq!(
        parse_bcfileformats(&write_bcfileformats(&[csv.clone()]).unwrap()).unwrap(),
        vec![csv]
    );
}
#[test]
fn empty_thousands_and_invalid_width_are_not_silently_lost() {
    let xml = r#"<BCFileFormats><TTableFormat Value="test"><DlmDelimiters Value="44"/><ThousandsSeparator Value=""/><FirstRow Value="Data"/></TTableFormat></BCFileFormats>"#;
    let formats = parse_bcfileformats(xml).unwrap();
    assert_eq!(formats[0].table.as_ref().unwrap().regional.thousands, None);
    assert_eq!(
        parse_bcfileformats(&write_bcfileformats(&formats).unwrap()).unwrap(),
        formats
    );
    assert!(parse_bcfileformats(
        &xml.replace("<FirstRow", "<FixedWidths Value=\"0;5\"/><FirstRow")
    )
    .is_err());
}
