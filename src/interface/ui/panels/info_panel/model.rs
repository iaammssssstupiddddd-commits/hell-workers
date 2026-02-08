use crate::entities::damned_soul::Gender;
use crate::interface::ui::presentation::EntityInspectionModel;

#[derive(Clone, PartialEq)]
pub(super) enum InfoPanelViewModel {
    Soul(SoulInfoViewModel),
    Simple(SimpleInfoViewModel),
}

#[derive(Clone, PartialEq)]
pub(super) struct SoulInfoViewModel {
    pub(super) header: String,
    pub(super) gender: Option<Gender>,
    pub(super) motivation: String,
    pub(super) stress: String,
    pub(super) fatigue: String,
    pub(super) task: String,
    pub(super) inventory: String,
    pub(super) common: String,
}

#[derive(Clone, PartialEq)]
pub(super) struct SimpleInfoViewModel {
    pub(super) header: String,
    pub(super) common: String,
}

pub(super) fn to_view_model(model: EntityInspectionModel) -> InfoPanelViewModel {
    if let Some(soul) = model.soul {
        InfoPanelViewModel::Soul(SoulInfoViewModel {
            header: model.header,
            gender: soul.gender,
            motivation: soul.motivation,
            stress: soul.stress,
            fatigue: soul.fatigue,
            task: soul.task,
            inventory: soul.inventory,
            common: soul.common,
        })
    } else {
        InfoPanelViewModel::Simple(SimpleInfoViewModel {
            header: model.header,
            common: model.common_text,
        })
    }
}
