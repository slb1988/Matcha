use bc_core::{align::AlignStrategy, encoding::Encoding};
use bc_engine_table::{
    columns::Side,
    rules::{ColumnHandling, ColumnMode, ColumnType, KeySelection},
    sheets::SheetPairs,
    source::ReadOptions,
    TableRules,
};
use bc_formats::model::{DateOrder, HeaderMode, TableType};
use eframe::egui;
#[derive(Clone)]
pub struct Draft {
    pub rules: TableRules,
    pub options: [ReadOptions; 2],
    pub pairs: SheetPairs,
    pub paths: [String; 2],
    pub readonly: [bool; 2],
    pub left_source: bool,
}
#[derive(Default)]
pub struct SettingsPanel {
    pub open: bool,
    pub draft: Option<Draft>,
    pub(crate) tab: usize,
    pub(crate) selected: usize,
    pub(crate) sheet: usize,
    pub(crate) move_side: usize,
    pub(crate) format_side: usize,
    pub(crate) description: String,
    pub(crate) error: String,
}
fn handling(ui: &mut egui::Ui, h: &mut ColumnHandling, default: bool) {
    if !default {
        ui.checkbox(&mut h.use_default, "Use Default");
    }
    ui.add_enabled_ui(default || !h.use_default, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut h.column_type, ColumnType::General, "General");
            ui.selectable_value(&mut h.column_type, ColumnType::Text, "Text");
        });
        ui.checkbox(&mut h.unimportant, "Unimportant");
        ui.checkbox(&mut h.ignore_case, "Character case (ABC = abc)");
        ui.checkbox(&mut h.ignore_whitespace, "Whitespace");
        ui.horizontal(|ui| {
            ui.label("Numeric tolerance");
            ui.add(
                egui::DragValue::new(&mut h.numeric_tolerance)
                    .range(0.0..=1.0e12)
                    .speed(0.001),
            );
        });
        let mut hours = h.date_tolerance.0 as f64 / 3_600_000.0;
        ui.horizontal(|ui| {
            ui.label("Date tolerance (hours; 24 = one day)");
            if ui
                .add(
                    egui::DragValue::new(&mut hours)
                        .range(0.0..=1.0e8)
                        .speed(0.1),
                )
                .changed()
            {
                h.date_tolerance.0 = (hours * 3_600_000.0).round() as i64;
            }
        });
    });
}
pub fn format_options(ui: &mut egui::Ui, options: &mut ReadOptions, error: &mut String) {
    let f = &mut options.format;
    ui.horizontal(|ui| {
        ui.selectable_value(&mut f.table_type, TableType::Delimited, "Delimited");
        ui.selectable_value(&mut f.table_type, TableType::Fixed, "Fixed");
    });
    if f.table_type == TableType::Delimited {
        ui.checkbox(&mut f.detect_delimiter, "Detect delimiter");
        ui.horizontal_wrapped(|ui| {
            for (c, label) in [
                (',', "Comma"),
                ('\t', "Tab"),
                (';', "Semicolon"),
                (' ', "Space"),
                ('|', "Pipe"),
            ] {
                let mut set = f.delimiters.contains(&c);
                if ui.checkbox(&mut set, label).changed() {
                    f.delimiters.retain(|&d| d != c);
                    if set {
                        f.delimiters.push(c);
                    }
                }
            }
        });
        let mut custom = f
            .delimiters
            .iter()
            .filter(|c| ![',', '\t', ';', ' ', '|'].contains(c))
            .collect::<String>();
        ui.horizontal(|ui| {
            ui.label("Custom delimiters");
            if ui.text_edit_singleline(&mut custom).changed() {
                f.delimiters
                    .retain(|c| [',', '\t', ';', ' ', '|'].contains(c));
                f.delimiters.extend(custom.chars());
            }
        });
        egui::ComboBox::from_id_salt("qualifier")
            .selected_text(f.qualifier.map_or("None".into(), |c| c.to_string()))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut f.qualifier, Some('"'), "Double quote");
                ui.selectable_value(&mut f.qualifier, Some('\''), "Single quote");
                ui.selectable_value(&mut f.qualifier, None, "None");
            });
        ui.checkbox(
            &mut f.consecutive_delimiters,
            "Treat consecutive delimiters as one",
        );
    } else {
        let mut widths = f
            .column_widths
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(";");
        ui.horizontal(|ui| {
            ui.label("Column widths (characters; e.g. 5;10;3)");
            if ui.text_edit_singleline(&mut widths).changed() {
                match widths
                    .split(';')
                    .map(|s| s.trim().parse::<usize>())
                    .collect::<Result<Vec<_>, _>>()
                {
                    Ok(w) if !w.contains(&0) => {
                        f.column_widths = w;
                        error.clear();
                    }
                    _ => *error = "Column widths must be positive integers".into(),
                }
            }
        });
    }
    ui.checkbox(
        &mut f.trim_surrounding_whitespace,
        "Surrounding whitespace belongs to delimiter",
    );
    ui.horizontal(|ui| {
        ui.label("First row");
        for (v, label) in [
            (HeaderMode::Auto, "Auto"),
            (HeaderMode::Data, "Data"),
            (HeaderMode::Names, "Column names"),
        ] {
            ui.selectable_value(&mut f.header, v, label);
        }
    });
    egui::ComboBox::from_id_salt("table_encoding")
        .selected_text(format!("{:?}", options.encoding))
        .show_ui(ui, |ui| {
            for (v, label) in [
                (None, "Detected"),
                (Some(Encoding::Utf8), "UTF-8"),
                (Some(Encoding::Utf16Le), "UTF-16 LE"),
                (Some(Encoding::Utf16Be), "UTF-16 BE"),
                (Some(Encoding::CodePage(1252)), "Windows-1252"),
                (Some(Encoding::CodePage(936)), "GBK / 936"),
            ] {
                ui.selectable_value(&mut options.encoding, v, label);
            }
        });
    ui.checkbox(&mut options.ctrl_z_eof, "Ctrl+Z indicates end of file");
    ui.separator();
    ui.label("Regional");
    ui.checkbox(
        &mut f.regional.use_system,
        "Use host regional settings (invariant unless host supplied)",
    );
    ui.add_enabled_ui(!f.regional.use_system, |ui| {
        for (name, c) in [
            ("Decimal separator", &mut f.regional.decimal),
            ("Date separator", &mut f.regional.date_separator),
        ] {
            let mut value = c.to_string();
            ui.horizontal(|ui| {
                ui.label(name);
                if ui.text_edit_singleline(&mut value).changed() {
                    if let Some(v) = value.chars().next() {
                        *c = v;
                    }
                }
            });
        }
        let mut thousands = f
            .regional
            .thousands
            .map_or(String::new(), |c| c.to_string());
        ui.horizontal(|ui| {
            ui.label("Thousands separator (empty = none)");
            if ui.text_edit_singleline(&mut thousands).changed() {
                f.regional.thousands = thousands.chars().next();
            }
        });
        ui.horizontal(|ui| {
            for (order, label) in [
                (DateOrder::Ymd, "YMD"),
                (DateOrder::Mdy, "MDY"),
                (DateOrder::Dmy, "DMY"),
            ] {
                ui.selectable_value(&mut f.regional.date_order, order, label);
            }
        });
    });
}
impl SettingsPanel {
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        names: &[Vec<String>; 2],
        key_hint: &str,
    ) -> Option<Draft> {
        if !self.open {
            return None;
        }
        let draft = self.draft.as_mut()?;
        let mut result = None;
        egui::Window::new("Table Session Settings").open(&mut self.open).default_size([720.0,620.0]).vscroll(true).show(ctx,|ui|{
            ui.horizontal(|ui|{for (i,name) in ["Specs","Format","Columns","Alignment","Sheets"].iter().enumerate(){ui.selectable_value(&mut self.tab,i,*name);}});
            ui.separator();
            match self.tab{
                0=>{
                    for s in 0..2{ui.push_id(s,|ui|{ui.label(if s==0{"Left file"}else{"Right file"});ui.text_edit_singleline(&mut draft.paths[s]);ui.checkbox(&mut draft.readonly[s],"Disable editing");});}
                    ui.label("Description (current session)");ui.text_edit_multiline(&mut self.description);
                }
                1=>{
                    ui.horizontal(|ui|{ui.selectable_value(&mut self.format_side,0,"Left");ui.selectable_value(&mut self.format_side,1,"Right");});
                    egui::ComboBox::from_id_salt("table_builtin_format").selected_text("Choose table format preset").show_ui(ui,|ui|{
                        for f in bc_formats::builtin_formats().iter().filter(|f|f.table.is_some()){
                            if ui.button(&f.name).clicked(){draft.options[self.format_side].format=f.table.clone().unwrap();ui.close();}
                        }
                    });
                    format_options(ui,&mut draft.options[self.format_side],&mut self.error);
                }
                2=>{
                    ui.label(format!("Keys: {} ({}) — comparison-column order defines priority",draft.rules.keys.label(),key_hint));
                    ui.horizontal(|ui|{
                        if ui.button("Auto infer keys").clicked(){for c in &mut draft.rules.columns.0{c.handling.is_key=false;}draft.rules.keys=KeySelection::Automatic{columns:Vec::new(),score:0.0};}
                        if ui.button("All columns").clicked(){for c in &mut draft.rules.columns.0{c.handling.is_key=false;}draft.rules.keys=KeySelection::AllColumns;}
                        if ui.button("By column number").clicked(){draft.rules.keys=KeySelection::ByColumnNumber;draft.rules.column_mode=ColumnMode::ByNumber;draft.rules.columns.0.clear();}
                        if ui.button("Detect column structure").clicked(){draft.rules.column_mode=ColumnMode::Automatic;draft.rules.columns.0.clear();}
                    });
                    egui::ScrollArea::vertical().id_salt("table_columns").max_height(180.0).show(ui,|ui|{
                        for (i,c) in draft.rules.columns.0.iter().enumerate(){if ui.selectable_label(i==self.selected,format!("{}: L {} ↔ R {}  {:?}",i+1,c.left.map_or("—".into(),|i|(i+1).to_string()),c.right.map_or("—".into(),|i|(i+1).to_string()),c.changes)).clicked(){self.selected=i;}}
                    });
                    ui.horizontal(|ui|{
                        for (i,n) in ["Whole comparison column","Left","Right"].iter().enumerate(){ui.selectable_value(&mut self.move_side,i,*n);}
                    });
                    ui.horizontal(|ui|{
                        let side=match self.move_side{1=>Some(Side::Left),2=>Some(Side::Right),_=>None};
                        if ui.button("Move Up").clicked()&&self.selected>0{let _=draft.rules.columns.move_to(self.selected,self.selected-1,side);self.selected-=1;}
                        if ui.button("Move Down").clicked()&&self.selected+1<draft.rules.columns.0.len(){let _=draft.rules.columns.move_to(self.selected,self.selected+1,side);self.selected+=1;}
                        if ui.button("Tidy").clicked(){draft.rules.columns.tidy();}
                        if ui.button("Accept structure").clicked(){for c in &mut draft.rules.columns.0{c.changes.clear();}}
                    });
                    let mut key=draft.rules.key_columns().contains(&self.selected);
                    if ui.checkbox(&mut key,"Key").changed(){
                        let mut keys=draft.rules.key_columns();keys.retain(|&c|c!=self.selected);if key{keys.push(self.selected);keys.sort_unstable();}let _=draft.rules.set_manual_keys(keys);
                    }
                    if let Some(c)=draft.rules.columns.0.get_mut(self.selected){
                        ui.horizontal(|ui|{for (name,index) in [("Left column (0 = gap)",&mut c.left),("Right column (0 = gap)",&mut c.right)]{
                            let mut i=index.map_or(0,|i|i+1);ui.label(name);if ui.add(egui::DragValue::new(&mut i).range(0..=10000)).changed(){*index=i.checked_sub(1);}
                        }});
                        handling(ui,&mut c.handling,false);
                    }
                    ui.collapsing("Default Handling",|ui|handling(ui,&mut draft.rules.default_handling,true));
                    ui.checkbox(&mut draft.rules.format_important,"Format differences are important")
                        .on_hover_text("Applies to the + Number formats layer; values/formulas layers are unaffected");
                }
                3=>{
                    ui.checkbox(&mut draft.rules.sort_by_key,"Sort rows before alignment");
                    for (strategy,name) in [(AlignStrategy::Smart,"Standard"),(AlignStrategy::Quick,"Myers O(ND)"),(AlignStrategy::Patience,"Patience"),(AlignStrategy::None,"Unaligned")]{ui.radio_value(&mut draft.rules.strategy,strategy,name);}
                    ui.checkbox(&mut draft.rules.use_closeness,"Use closeness matching");
                    if ui.checkbox(&mut draft.left_source,"Left file is source").changed(){draft.rules.left_is_source=draft.left_source;draft.pairs=SheetPairs::by_name(&names[0],&names[1],draft.left_source);}
                }
                _=>{
                    ui.label("Move one side to compare different sheets, including the same file twice.");
                    for (i,p) in draft.pairs.0.iter().enumerate(){
                        if ui.selectable_label(i==self.sheet,format!("{} ↔ {}",p.left.and_then(|i|names[0].get(i)).map_or("—",String::as_str),p.right.and_then(|i|names[1].get(i)).map_or("—",String::as_str))).clicked(){self.sheet=i;}
                    }
                    ui.horizontal(|ui|{ui.selectable_value(&mut self.move_side,1,"Left");ui.selectable_value(&mut self.move_side,2,"Right");
                        let side=if self.move_side==1{Side::Left}else{Side::Right};
                        if ui.button("Move Up").clicked()&&self.sheet>0{let _=draft.pairs.move_to(self.sheet,self.sheet-1,side);self.sheet-=1;}
                        if ui.button("Move Down").clicked()&&self.sheet+1<draft.pairs.0.len(){let _=draft.pairs.move_to(self.sheet,self.sheet+1,side);self.sheet+=1;}
                    });
                }
            }
            ui.separator();if ui.button("Apply to current comparison").clicked(){result=Some(draft.clone());}
            if !self.error.is_empty(){ui.colored_label(egui::Color32::RED,&self.error);}
        });
        result
    }
}
