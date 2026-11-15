//! Per-operation scope review happens before a plan is built, not after I/O starts.
use crate::chrome::{controls::Role, widgets, window::Window};
use crate::egui;
use bc_core::Side;
use bc_engine_dir::ops::DirOp;
pub struct ScopeDialog{pub op:DirOp,pub include_hidden:bool,pub choose_side:bool,pub destination:usize}
impl ScopeDialog{
    pub fn new(op:DirOp,include_hidden:bool,choose_side:bool)->Self{Self{op,include_hidden,choose_side,destination:1}}
    pub fn show(&mut self,ctx:&egui::Context)->Option<Result<(DirOp,bool),()>>{
        let mut result=None;let mut open=true;
        Window::new("Review operation scope").open(&mut open).show(ctx,|ui|{
            ui.label(format!("{:?}",self.op));
            widgets::checkbox(ui,&mut self.include_hidden,"Include hidden / filtered contents");
            if self.choose_side{
                ui.horizontal(|ui|{ui.label("Destination");widgets::radio_value(ui,&mut self.destination,0,"Left");widgets::radio_value(ui,&mut self.destination,1,"Right");});
            }
            widgets::hint(ui,"Overwrite, newer-target, read-only and deletion confirmations follow separately.");
            let choice=widgets::dialog_buttons(ui,&[("Cancel",Role::Secondary),("Build plan and continue",Role::Primary)]);
            {
                if choice==Some(1){
                    let(from,to)=if self.destination==0{(Side::Right,Side::Left)}else{(Side::Left,Side::Right)};
                    let op=if self.choose_side{match self.op{DirOp::CopyOther=>DirOp::Copy{from,to},DirOp::MoveOther=>DirOp::Move{from,to},_=>self.op.clone()}}else{self.op.clone()};
                    result=Some(Ok((op,self.include_hidden)));
                }
                if choice==Some(0){result=Some(Err(()));}
            }
        });if !open{result=Some(Err(()));}result
    }
}
