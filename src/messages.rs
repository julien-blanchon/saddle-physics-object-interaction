use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Debug, PartialEq)]
pub enum AcquireFailureReason {
    InteractorDisabled,
    NoValidTarget,
    InvalidExplicitTarget,
    TargetTooFar,
    TargetBlocked,
    TargetTooHeavy,
    TargetAlreadyHeld,
    TargetNotDynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Debug, PartialEq)]
pub enum ReleaseReason {
    Dropped,
    Thrown,
    Deactivated,
    DistanceExceeded,
    Occluded,
    Unstable,
    TargetInvalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Debug, PartialEq)]
pub enum CycleDirection {
    Next,
    Previous,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct TryAcquireObject {
    pub interactor: Entity,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct SetInteractionTarget {
    pub interactor: Entity,
    pub target: Option<Entity>,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct ReleaseHeldObject {
    pub interactor: Entity,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct ThrowHeldObject {
    pub interactor: Entity,
    pub impulse_scale: f32,
    pub angular_impulse_scale: f32,
}

impl Default for ThrowHeldObject {
    fn default() -> Self {
        Self {
            interactor: Entity::PLACEHOLDER,
            impulse_scale: 1.0,
            angular_impulse_scale: 1.0,
        }
    }
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct AdjustHoldDistance {
    pub interactor: Entity,
    pub delta: f32,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct RotateHeldObject {
    pub interactor: Entity,
    pub delta: Quat,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct SetSurfacePlacementMode {
    pub interactor: Entity,
    pub enabled: bool,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct CycleInteractionTarget {
    pub interactor: Entity,
    pub direction: CycleDirection,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct ObjectAcquired {
    pub interactor: Entity,
    pub object: Entity,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct ObjectReleased {
    pub interactor: Entity,
    pub object: Entity,
    pub reason: ReleaseReason,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct ObjectThrown {
    pub interactor: Entity,
    pub object: Entity,
    pub impulse: Vec3,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct ObjectInteractionFailed {
    pub interactor: Entity,
    pub target: Option<Entity>,
    pub reason: AcquireFailureReason,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Debug, PartialEq)]
pub struct HeldObjectBecameUnstable {
    pub interactor: Entity,
    pub object: Entity,
    pub error_distance: f32,
}
