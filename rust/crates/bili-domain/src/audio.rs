//! 音频播放器的语言中立实体。protobuf 消息止步数据层。

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioPlayItem {
    pub item_type: i32,
    pub oid: i64,
    pub sub_id: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioArchive {
    pub title: String,
    pub cover: String,
    pub desc: String,
    pub duration: i64,
    pub publish: i64,
    pub displayed_oid: String,
    pub copyright: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioPart {
    pub oid: i64,
    pub sub_id: i64,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioOwner {
    pub mid: i64,
    pub name: String,
    pub avatar: String,
    pub has_name: bool,
    pub has_avatar: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioStat {
    pub like: i32,
    pub coin: i32,
    pub favourite: i32,
    pub reply: i32,
    pub share: i32,
    pub view: i32,
    pub has_like: bool,
    pub has_coin: bool,
    pub has_fav: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDetail {
    pub item: AudioPlayItem,
    pub arc: AudioArchive,
    pub parts: Vec<AudioPart>,
    pub owner: AudioOwner,
    pub stat: AudioStat,
    pub associated_item: AudioPlayItem,
    pub has_associated_oid: bool,
    pub has_desc: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioPlaylist {
    pub reach_start: bool,
    pub reach_end: bool,
    pub prev: String,
    pub next: String,
    pub list: Vec<AudioDetail>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioVolume {
    pub measured_i: f64,
    pub measured_lra: f64,
    pub measured_tp: f64,
    pub measured_threshold: f64,
    pub target_offset: f64,
    pub target_i: f64,
    pub target_tp: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioPlayback {
    pub url: Option<String>,
    pub volume: Option<AudioVolume>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioWriteReply {
    pub message: String,
    pub coin_ok: bool,
}
