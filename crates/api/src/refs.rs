//! Shared response shapes for things one resource says about another.

use lodestar_core::project::{ItemRef, PersonRef};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// A Person as shown on something that refers to them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[schema(as = PersonRef)]
pub struct PersonRefBody {
    pub id: Uuid,
    pub display_name: String,
}

impl From<PersonRef> for PersonRefBody {
    fn from(person: PersonRef) -> Self {
        Self {
            id: person.id.as_uuid(),
            display_name: person.display_name,
        }
    }
}

/// Another item, by ID and Reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[schema(as = ItemRef)]
pub struct ItemRefBody {
    pub id: Uuid,
    /// e.g. `A-001`.
    pub reference: String,
}

impl From<ItemRef> for ItemRefBody {
    fn from(item: ItemRef) -> Self {
        Self {
            id: item.id,
            reference: item.reference,
        }
    }
}
