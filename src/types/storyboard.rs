#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryboardLayer {
    Background,
    Fail,
    Pass,
    Foreground,
    Overlay,
}
impl StoryboardLayer {
    pub fn from_str(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "background" => Some(Self::Background),
            "fail" => Some(Self::Fail),
            "pass" => Some(Self::Pass),
            "foreground" => Some(Self::Foreground),
            "overlay" => Some(Self::Overlay),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryboardOrigin {
    TopLeft,
    TopCentre,
    TopRight,
    CentreLeft,
    Centre,
    CentreRight,
    BottomLeft,
    BottomCentre,
    BottomRight,
}
impl StoryboardOrigin {
    pub fn from_str(raw: &str) -> Option<Self> {
        let lower = raw.trim().to_lowercase();
        match lower.as_str() {
            "topleft" => Some(Self::TopLeft),
            "topcentre" | "topcenter" => Some(Self::TopCentre),
            "topright" => Some(Self::TopRight),
            "centreleft" | "centerleft" => Some(Self::CentreLeft),
            "centre" | "center" => Some(Self::Centre),
            "centreright" | "centerright" => Some(Self::CentreRight),
            "bottomleft" => Some(Self::BottomLeft),
            "bottomcentre" | "bottomcenter" => Some(Self::BottomCentre),
            "bottomright" => Some(Self::BottomRight),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationLoopType {
    LoopForever,
    LoopOnce,
}
impl AnimationLoopType {
    pub fn from_str(raw: &str) -> Self {
        match raw.trim().to_lowercase().as_str() {
            "looponce" => Self::LoopOnce,
            _ => Self::LoopForever,
        }
    }
}
#[derive(Debug, Clone)]
pub enum StoryboardObjectKind {
    Sprite {
        filepath: String,
        x: f32,
        y: f32,
    },
    Animation {
        filepath: String,
        x: f32,
        y: f32,
        frame_count: u32,
        frame_delay: u32,
        loop_type: AnimationLoopType,
    },
}
#[derive(Debug, Clone)]
pub struct StoryboardObject {
    pub layer: StoryboardLayer,
    pub origin: StoryboardOrigin,
    pub kind: StoryboardObjectKind,
    pub commands: Vec<StoryboardCommand>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryboardCommandKind {
    Fade,
    Move,
    MoveX,
    MoveY,
    Scale,
    VectorScale,
    Rotate,
    Color,
    Param,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StoryboardParamFlags {
    pub h: bool,
    pub v: bool,
    pub a: bool,
}
impl StoryboardParamFlags {
    pub fn from_str(raw: &str) -> Self {
        let mut flags = Self::default();
        for ch in raw.chars() {
            // Storyboard P commands use H/V/A for horizontal flip, vertical flip, and additive blending.
            match ch.to_ascii_lowercase() {
                'h' => flags.h = true,
                'v' => flags.v = true,
                'a' => flags.a = true,
                _ => {}
            }
        }
        flags
    }
}
#[derive(Debug, Clone)]
pub struct StoryboardCommandData {
    pub kind: StoryboardCommandKind,
    pub easing: i32,
    pub start_time: i32,
    pub end_time: i32,
    pub start_values: Vec<f32>,
    pub end_values: Vec<f32>,
    pub params: StoryboardParamFlags,
}
#[derive(Debug, Clone)]
pub struct StoryboardLoop {
    pub start_time: i32,
    pub loop_count: i32,
    pub commands: Vec<StoryboardCommand>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleSet {
    Normal,
    Soft,
    Drum,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitSoundType {
    Whistle,
    Finish,
    Clap,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct StoryboardTriggerKind {
    pub any: bool,
    pub sample_set: Option<SampleSet>,
    pub hit_sound: Option<HitSoundType>,
}
impl StoryboardTriggerKind {
    pub fn from_name(raw: &str) -> Option<Self> {
        let name = raw.trim();
        if name.is_empty() {
            return None;
        }
        let lower = name.to_lowercase();
        if !lower.starts_with("hitsound") {
            return None;
        }
        let rest = lower.trim_start_matches("hitsound");
        if rest.is_empty() {
            // "HitSound" without a suffix triggers on any hitsound event.
            return Some(Self {
                any: true,
                ..Default::default()
            });
        }
        let mut trigger = Self {
            any: false,
            ..Default::default()
        };
        if rest.contains("soft") {
            // Trigger names concatenate sample set and hit sound, such as HitSoundSoftWhistle.
            trigger.sample_set = Some(SampleSet::Soft);
        } else if rest.contains("drum") {
            trigger.sample_set = Some(SampleSet::Drum);
        } else if rest.contains("normal") {
            trigger.sample_set = Some(SampleSet::Normal);
        }
        if rest.contains("whistle") {
            trigger.hit_sound = Some(HitSoundType::Whistle);
        } else if rest.contains("finish") {
            trigger.hit_sound = Some(HitSoundType::Finish);
        } else if rest.contains("clap") {
            trigger.hit_sound = Some(HitSoundType::Clap);
        }
        if trigger.sample_set.is_none() && trigger.hit_sound.is_none() {
            return None;
        }
        Some(trigger)
    }
}
#[derive(Debug, Clone)]
pub struct StoryboardTrigger {
    pub trigger: StoryboardTriggerKind,
    pub start_time: i32,
    pub end_time: i32,
    pub commands: Vec<StoryboardCommand>,
}
#[derive(Debug, Clone)]
pub enum StoryboardCommand {
    Command(StoryboardCommandData),
    Loop(StoryboardLoop),
    Trigger(StoryboardTrigger),
}
#[derive(Debug, Clone, Default)]
pub struct Storyboard {
    pub objects: Vec<StoryboardObject>,
}
impl Storyboard {
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}
