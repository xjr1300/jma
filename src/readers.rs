use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use time::{Date, Month, PrimitiveDateTime, Time};

type FileReader = BufReader<File>;

/// `RapReader`
#[derive(Debug)]
pub struct RapReader {
    /// パス
    path: Rc<PathBuf>,
    /// コメント
    comment_part: CommentPart,
    /// データ部へのインデックス
    data_index_part: DataIndexPart,
    /// 格子系定義
    grid_definition_part: GridDefinitionPart,
    /// 圧縮方法、観測値表
    compression_part: CompressionPart,
    /// レベル反復数表
    level_repetitions_part: LevelRepetitionsPart,
}

impl RapReader {
    /// RAPファイルを開く
    ///
    /// # 引数
    ///
    /// * `path` - 開くRAPファイルのパス
    ///
    /// # 戻り値
    ///
    /// `RapReader`
    pub fn new<P>(path: P) -> RapReaderResult<Self>
    where
        P: AsRef<Path>,
    {
        let path = Rc::new(path.as_ref().to_owned());
        let file = OpenOptions::new()
            .read(true)
            .open(Rc::clone(&path).as_path())
            .map_err(|e| RapReaderError::Open(format!("{e}")))?;
        let mut reader = BufReader::new(file);
        let comment_part = read_comment_part(&mut reader)?;
        let data_index_part = read_data_index_part(&mut reader)?;
        let grid_definition_part = read_grid_definition_part(&mut reader)?;
        let compression_part = read_compression_part(&mut reader)?;
        let level_repetitions_part = read_level_repetitions_part(&mut reader)?;

        Ok(Self {
            path,
            comment_part,
            data_index_part,
            grid_definition_part,
            compression_part,
            level_repetitions_part,
        })
    }

    /// 管理部 - コメント - 識別子を返す。
    pub fn identifier(&self) -> &str {
        &self.comment_part.identifier
    }

    /// 管理部 - コメント - 版番号を返す。
    pub fn version(&self) -> &str {
        &self.comment_part.version
    }

    /// 管理部 - コメント - 作成者コメントを返す。
    pub fn creator_comment(&self) -> &str {
        &self.comment_part.creator_comment
    }

    /// 管理部 - データ部へのインデックス - データ数を返す。
    pub fn number_of_data(&self) -> u32 {
        self.data_index_part.number_of_data as u32
    }

    /// 記録しているデータの属性を格納したスライスを返す。
    ///
    /// RAPファイルは、1つのファイルに1日分のデータを記録している。
    /// 1つのファイルには、1時間間隔で観測した24データ、または30分間隔で観測した48データが
    /// 記録されている。
    /// データ数は、`number_of_data`メソッドで確認できる。
    pub fn data_properties(&self) -> &[DataProperty] {
        &self.data_index_part.data_properties
    }

    /// 管理部 - 格子系定義 - 地図種別を返す。
    pub fn map_type(&self) -> u16 {
        self.grid_definition_part.map_type
    }

    /// 管理部 - 格子系定義 - 最北西端の緯度を10e-6度単位で返す。
    pub fn grid_start_latitude(&self) -> u32 {
        self.grid_definition_part.start_grid_latitude
    }

    /// 管理部 - 格子系定義 - 最北西端の経度を10e-6度単位で返す。
    pub fn grid_start_longitude(&self) -> u32 {
        self.grid_definition_part.start_grid_longitude
    }

    /// 管理部 - 格子系定義 - 格子の幅を10e-6度単位で返す。
    pub fn grid_width(&self) -> u32 {
        self.grid_definition_part.grid_width
    }

    /// 管理部 - 格子系定義 - 格子の高さを10e-6度単位で返す。
    pub fn grid_height(&self) -> u32 {
        self.grid_definition_part.grid_height
    }

    /// 管理部 - 格子系定義 - 観測範囲の経度方向の格子数を返す。
    pub fn number_of_h_grids(&self) -> u16 {
        self.grid_definition_part.number_of_h_grids
    }

    /// 管理部 - 格子系定義 - 観測範囲の緯度方向の格子数を返す。
    pub fn number_of_v_grids(&self) -> u16 {
        self.grid_definition_part.number_of_v_grids
    }

    /// 管理部 - 圧縮方法、観測値表 - 圧縮方法を返す。
    pub fn compression_method(&self) -> u16 {
        self.compression_part.compression_method
    }

    /// 管理部 - 圧縮方法、観測値表 - レベルの数を返す。
    pub fn number_of_levels(&self) -> u16 {
        self.compression_part.number_of_levels
    }

    /// 管理部 - 圧縮方法、観測値表 - レベル別の観測値を返す。
    pub fn value_by_levels(&self) -> &[u16] {
        &self.compression_part.value_by_levels
    }

    /// 管理部 - レベル、反復数表 - レベルと反復数の組み合わせの数を返す。
    pub fn number_of_level_repetitions(&self) -> u16 {
        self.level_repetitions_part.number_of_level_repetitions
    }

    /// 管理部 - レベル、反復数表 - レベルと反復数の組み合わせを返す。
    pub fn level_repetitions(&self) -> &[LevelRepetition] {
        &self.level_repetitions_part.level_repetitions
    }

    /// 引数で指定された日時の観測データの属性を返却する。
    ///
    /// # 引数
    ///
    /// * `dt` - 観測データの属性を取得したい日時
    ///
    /// # 戻り値
    ///
    /// 観測データの属性を格納した`DataAttribute`
    pub fn retrieve_observation_data(
        &mut self,
        dt: PrimitiveDateTime,
    ) -> RapReaderResult<DataPart<'_>> {
        let data_property = self
            .data_index_part
            .data_properties
            .iter()
            .find(|dp| dp.observation_date_time == dt)
            .ok_or(RapReaderError::DataDoesNotRecorded(dt))?;

        let file = OpenOptions::new()
            .read(true)
            .open(Rc::clone(&self.path).as_path())
            .map_err(|e| RapReaderError::Open(format!("{e}")))?;
        let mut reader = BufReader::new(file);
        reader
            .seek(SeekFrom::Start(data_property.data_start_position as u64))
            .map_err(|e| {
                RapReaderError::Unexpected(format!(
                    "データが記録されている位置へのシークに失敗しました。{e}"
                ))
            })?;

        self.read_observation_data_part(reader)
    }

    fn read_observation_data_part(&self, mut reader: FileReader) -> RapReaderResult<DataPart<'_>> {
        let compressed_data_bytes = read_u32(&mut reader).map_err(|e| {
            RapReaderError::Unexpected(format!(
                "データ部のデータ圧縮後のサイズの読み込みに失敗しました。{e}"
            ))
        })?;
        let compressed_data_start_position = reader.get_mut().stream_position().map_err(|e| {
            RapReaderError::Unexpected(format!(
                "圧縮されたデータの開始位置を取得できませんでした。{e}"
            ))
        })?;
        reader
            .seek(SeekFrom::Current(compressed_data_bytes as i64))
            .map_err(|e| {
                RapReaderError::Unexpected(format!(
                    "データ部の圧縮データのシークに失敗しました。{e}"
                ))
            })?;
        let mut radar_operation_statuses = [0u8; 8];
        reader
            .read_exact(&mut radar_operation_statuses)
            .map_err(|e| {
                RapReaderError::Unexpected(format!(
                    "データ部のレーダー運用状況の読み込みに失敗しました。{e}"
                ))
            })?;
        let number_of_amedases = read_u32(reader.get_mut()).map_err(|e| {
            RapReaderError::Unexpected(format!(
                "データ部の解析に使用したアメダスの総数の読み込みに失敗しました。{e}"
            ))
        })?;

        // 観測値を記録順に走査して返すイテレーターを構築
        let value_iterator = RapValueIterator::new(
            reader,
            compressed_data_bytes as usize,
            self.grid_start_latitude(),
            self.grid_start_longitude(),
            self.number_of_h_grids(),
            self.grid_height(),
            self.grid_width(),
            self.value_by_levels(),
            self.level_repetitions(),
        );

        Ok(DataPart {
            compressed_data_bytes,
            compressed_data_start_position,
            value_iterator,
            radar_operation_statuses,
            number_of_amedases,
        })
    }
}

/// コメント
#[derive(Debug, Clone)]
struct CommentPart {
    /// 識別子
    identifier: String,

    /// 版番号
    version: String,

    /// 作成者コメント
    creator_comment: String,
}

/// データ部へのインデックス
#[derive(Debug, Clone, Copy)]
pub struct DataProperty {
    /// 観測日時
    pub observation_date_time: PrimitiveDateTime,

    /// 観測要素
    pub observation_element: u16,

    /// 最初のデータが記録されているファイルの先頭からのバイト位置
    pub data_start_position: u32,
}

/// データ部へのインデックス
#[derive(Debug, Clone)]
struct DataIndexPart {
    /// データ数
    ///
    /// データ数が24の場合は、毎正時に観測したデータを記録したファイルを示し、
    /// データ数が48の場合は、30分毎に観測したデータを記録したファイルを示す。
    number_of_data: ObservationTimes,

    /// データの属性
    data_properties: Vec<DataProperty>,
}

/// 格子系定義
#[derive(Debug, Clone, Copy)]
struct GridDefinitionPart {
    /// 地図種別
    ///
    /// 1: 解析雨量
    map_type: u16,

    /// 最初の緯度と軽度
    ///
    /// 10e-6度単位で表現する。
    /// 最初のデータは観測範囲の北西端である。
    /// 最初のデータ以後は、経度方向に西から東にデータが記録され、東端に達したとき、
    /// 格子1つ分だけ南で、西端の格子のデータが記録されている。
    start_grid_latitude: u32,
    start_grid_longitude: u32,

    /// 横方向と縦方向の格子間隔
    ///
    /// 10e-6度単位で表現する。
    grid_width: u32,
    grid_height: u32,

    /// 横方向と縦方向の格子数
    pub(crate) number_of_h_grids: u16,
    pub(crate) number_of_v_grids: u16,
}

/// 圧縮方法、観測値表
#[derive(Debug, Clone)]
struct CompressionPart {
    /// 圧縮方法
    compression_method: u16,

    /// レベル数
    number_of_levels: u16,

    /// レベル毎の観測値
    ///
    /// レベルは`Vec`のインデックスを示す。
    value_by_levels: Vec<u16>,
}

/// レベルと反復数
#[derive(Debug, Clone, Copy, Default)]
pub struct LevelRepetition {
    /// レベル
    pub level: u8,

    /// 反復数
    ///
    /// 記録されている値は、実際の反復数より2少ない数を格納している。
    pub repetition: u8,
}

/// レベルと反復数表
#[derive(Debug, Clone)]
struct LevelRepetitionsPart {
    /// レベル反復数（繰り返し回数）
    ///
    /// 実際の反復回数は、要素+2回となる。
    /// レベルは`Vec`のインデックスを示す。
    pub(crate) number_of_level_repetitions: u16,

    // レベルと反復数の組み合わせ
    pub(crate) level_repetitions: Vec<LevelRepetition>,
}

/// データ部
pub struct DataPart<'a> {
    /// 圧縮後のデータのサイズ
    pub compressed_data_bytes: u32,

    /// 圧縮されたデータの開始位置
    pub compressed_data_start_position: u64,

    /// 観測値を順に走査して返すイテレーター
    ///
    /// 観測値を記録した格子は、最北西端から経度方向に向かって記録されている。
    /// 格子がその緯度の最東端に達したとき、現在の格子の1つ南かつ、最西端の格子に移動する。
    /// これを続けて最南東端の格子に移動する。
    pub value_iterator: RapValueIterator<'a>,

    /// レーダー運用状況
    pub radar_operation_statuses: [u8; 8],

    /// 解析に使用したアメダスの総数
    pub number_of_amedases: u32,
}

/// 1日の観測回数
#[derive(Debug, Clone, Copy)]
pub enum ObservationTimes {
    /// 24回
    ///
    /// 毎正時に観測（1時間間隔）
    Times24 = 24,

    /// 48回
    ///
    /// 30分毎に観測
    Times48 = 48,
}

/// `u8`型から1日の観測回数を示す`ObservationTimes`に変換する。
impl TryFrom<u32> for ObservationTimes {
    type Error = RapReaderError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            24 => Ok(Self::Times24),
            48 => Ok(Self::Times48),
            _ => Err(RapReaderError::ObservationIntervalUnsupported(value)),
        }
    }
}

/// 地図種別
const MAP_TYPE: u16 = 1; // 緯度・経度格子座標系

/// 圧縮方法
const COMPRESSION_METHOD: u16 = 1; // ラン・レングス符号圧縮

/// RapReaderエラー型
#[derive(Debug, Clone, thiserror::Error)]
pub enum RapReaderError {
    /// 予期しない例外
    #[error("{0}")]
    Unexpected(String),

    /// ファイル・オープン・エラー
    #[error("ファイルを開くときにエラーが発生しました。{0}")]
    Open(String),

    /// サポートしていない観測時間間隔
    #[error("サポートしていない時間間隔です。`{0}`")]
    ObservationIntervalUnsupported(u32),

    /// サポートしていない地図種別
    #[error("サポートしていない地図種別です。`{0}`")]
    MapTypeUnsupported(u16),

    /// サポートしていない圧縮方法
    #[error("サポートしていない圧縮方法です。`{0}`")]
    CompressionMethodUnsupported(u16),

    /// 指定された日付のデータが記録されていない
    #[error("指定された日付のデータは記録されていません。`{0:?}`")]
    DataDoesNotRecorded(PrimitiveDateTime),
}

/// RapReader結果型
pub type RapReaderResult<T> = Result<T, RapReaderError>;

/// 文字列を読み込む。
///
/// 読み込んだ文字列は、末尾の空白文字をトリムした結果である。
///
/// # 引数
///
/// * `reader` - 文字列を読み込むリーダー
/// * `bytes` - 読み込むバイト数
///
/// # 戻り値
///
/// 読み込んだ文字列
fn read_str<R>(reader: &mut R, bytes: usize) -> RapReaderResult<String>
where
    R: Read,
{
    let mut buf = vec![0u8; bytes];
    reader.read_exact(&mut buf).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "ファイルから{bytes}バイトの読み込みに失敗しました。{e}"
        ))
    })?;
    let s = String::from_utf8(buf).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "utf8文字列に変換できないバイト列が記録されています。{e}"
        ))
    })?;
    let s = s.trim_end().to_string();

    Ok(s)
}

macro_rules! read_number {
    ($func_name:ident, $type: ty) => {
        fn $func_name<R>(reader: &mut R) -> RapReaderResult<$type>
        where
            R: Read,
        {
            let bytes = std::mem::size_of::<$type>();
            let mut buf = vec![0u8; bytes];
            reader.read_exact(&mut buf).map_err(|e| {
                RapReaderError::Unexpected(format!(
                    "ファイルから{bytes}バイトの読み込みに失敗しました。{e}"
                ))
            })?;

            Ok(<$type>::from_le_bytes(buf.try_into().unwrap()))
        }
    };
}

read_number!(read_u8, u8);
read_number!(read_u16, u16);
read_number!(read_u32, u32);

fn read_date_time<R>(reader: &mut R) -> RapReaderResult<PrimitiveDateTime>
where
    R: Read,
{
    let year = read_u16(reader)
        .map_err(|e| RapReaderError::Unexpected(format!("観測年の読み込みに失敗しました。{e}")))?;
    let month = read_u8(reader)
        .map_err(|e| RapReaderError::Unexpected(format!("観測月の読み込みに失敗しました。{e}")))?;
    let month_enum = Month::try_from(month).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "ファイルに記録されている月({month})が不正です。{e}"
        ))
    })?;
    let day = read_u8(reader)
        .map_err(|e| RapReaderError::Unexpected(format!("観測日の読み込みに失敗しました。{e}")))?;
    let hour = read_u8(reader)
        .map_err(|e| RapReaderError::Unexpected(format!("観測時の読み込みに失敗しました。{e}")))?;
    let minute = read_u8(reader)
        .map_err(|e| RapReaderError::Unexpected(format!("観測分の読み込みに失敗しました。{e}")))?;
    let date = Date::from_calendar_date(year as i32, month_enum, day).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "ファイルに記録されている年月日から、日付を構築できませんでした。{e}"
        ))
    })?;
    let time = Time::from_hms(hour, minute, 0).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "ファイルに記録されている時分から、時間を構築できませんでした。{e}"
        ))
    })?;

    Ok(PrimitiveDateTime::new(date, time))
}

fn read_comment_part<R>(reader: &mut R) -> RapReaderResult<CommentPart>
where
    R: Read + Seek,
{
    let identifier = read_str(reader, 6).map_err(|e| {
        RapReaderError::Unexpected(format!("コメントの識別子の読み込みに失敗しました。{e}"))
    })?;
    let version = read_str(reader, 5).map_err(|e| {
        RapReaderError::Unexpected(format!("コメントの版番号の読み込みに失敗しました。{e}"))
    })?;
    let comment = read_str(reader, 66).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "コメントの作成者コメントの読み込みに失敗しました。{e}"
        ))
    })?;
    let mut bytes = [0u8; 3];
    reader.read_exact(&mut bytes).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "コメントの末尾3バイトの読み込みに失敗しました。{e}"
        ))
    })?;
    if bytes != [0x0d, 0x0a, 0x00] {
        return Err(RapReaderError::Unexpected(format!(
            "コメントの末尾3バイトが`0x0d 0x0a 0x00`ではありません。実際には{:?}でした。",
            bytes,
        )));
    }

    Ok(CommentPart {
        identifier,
        version,
        creator_comment: comment,
    })
}

fn read_data_index_part<R>(reader: &mut R) -> RapReaderResult<DataIndexPart>
where
    R: Read + Seek,
{
    let number_of_data = read_u32(reader).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "データ部へのインデックスのデータ数の読み込みに失敗しました。{e}"
        ))
    })?;
    let number_of_data = ObservationTimes::try_from(number_of_data)?;
    let mut data_properties = vec![
        DataProperty {
            observation_date_time: PrimitiveDateTime::MIN,
            observation_element: 0,
            data_start_position: 0,
        };
        number_of_data as usize
    ];
    for data_property in data_properties.iter_mut() {
        data_property.observation_date_time = read_date_time(reader)?;
        data_property.observation_element = read_u16(reader).map_err(|e| {
            RapReaderError::Unexpected(format!(
                "データ部へのインデックスの要素の読み込みに失敗しました。{e}"
            ))
        })?;
        reader.seek(SeekFrom::Current(8)).map_err(|e| {
            RapReaderError::Unexpected(format!(
                "データ部へのインデックスの予備のシークに失敗しました。{e}"
            ))
        })?;
        data_property.data_start_position = read_u32(reader).map_err(|e| {
            RapReaderError::Unexpected(format!(
                "データ部へのインデックスのデータの開始位置の読み込みに失敗しました。{e}"
            ))
        })?;
    }

    Ok(DataIndexPart {
        number_of_data,
        data_properties,
    })
}

fn read_grid_definition_part<R>(reader: &mut R) -> RapReaderResult<GridDefinitionPart>
where
    R: Read + Seek,
{
    reader.seek(SeekFrom::Current(2)).map_err(|e| {
        RapReaderError::Unexpected(format!("格子系定義の最初の予備のシークに失敗しました。{e}"))
    })?;
    let map_type = read_u16(reader).map_err(|e| {
        RapReaderError::Unexpected(format!("格子系定義の地図種別の読み込みに失敗しました。{e}"))
    })?;
    if map_type != MAP_TYPE {
        return Err(RapReaderError::MapTypeUnsupported(map_type));
    }
    let start_grid_latitude = read_u32(reader).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "格子系定義の最初のデータの緯度の読み込みに失敗しました。{e}"
        ))
    })?;
    let start_grid_longitude = read_u32(reader).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "格子系定義の最初のデータの経度の読み込みに失敗しました。{e}"
        ))
    })?;
    let grid_width = read_u32(reader).map_err(|e| {
        RapReaderError::Unexpected(format!("格子系定義の格子の幅の読み込みに失敗しました。{e}"))
    })?;
    let grid_height = read_u32(reader).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "格子系定義の格子の高さの読み込みに失敗しました。{e}"
        ))
    })?;
    let number_of_h_grids = read_u16(reader).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "格子系定義の横方向の格子数の読み込みに失敗しました。{e}"
        ))
    })?;
    let number_of_v_grids = read_u16(reader).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "格子系定義の縦方向の格子数の読み込みに失敗しました。{e}"
        ))
    })?;
    reader.seek(SeekFrom::Current(16)).map_err(|e| {
        RapReaderError::Unexpected(format!("格子系定義の最後の予備のシークに失敗しました。{e}"))
    })?;

    Ok(GridDefinitionPart {
        map_type,
        start_grid_latitude,
        start_grid_longitude,
        grid_width,
        grid_height,
        number_of_h_grids,
        number_of_v_grids,
    })
}

fn read_compression_part<R>(reader: &mut R) -> RapReaderResult<CompressionPart>
where
    R: Read,
{
    let compression_method = read_u16(reader).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "圧縮方法・観測値表の圧縮方法の読み込みに失敗しました。{e}"
        ))
    })?;
    if compression_method != COMPRESSION_METHOD {
        return Err(RapReaderError::CompressionMethodUnsupported(
            compression_method,
        ));
    }
    let number_of_levels = read_u16(reader).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "圧縮方法・観測値表のレベル数の読み込みに失敗しました。{e}"
        ))
    })?;
    let mut value_by_levels = vec![0u16, number_of_levels];
    for prep in value_by_levels.iter_mut() {
        *prep = read_u16(reader).map_err(|e| {
            RapReaderError::Unexpected(format!(
                "圧縮方法・観測値表のレベルごとの観測値の読み込みに失敗しました。{e}"
            ))
        })?;
    }

    Ok(CompressionPart {
        compression_method,
        number_of_levels,
        value_by_levels,
    })
}

fn read_level_repetitions_part<R>(reader: &mut R) -> RapReaderResult<LevelRepetitionsPart>
where
    R: Read,
{
    let number_of_level_repetitions = read_u16(reader).map_err(|e| {
        RapReaderError::Unexpected(format!(
            "レベル・反復表の表の大きさの読み込みに失敗しました。{e}"
        ))
    })?;
    let mut level_repetitions = vec![
        LevelRepetition {
            level: 0,
            repetition: 0
        };
        number_of_level_repetitions as usize
    ];
    for lr in level_repetitions.iter_mut() {
        lr.level = read_u8(reader).map_err(|e| {
            RapReaderError::Unexpected(format!(
                "レベル・反復表のレベルの読み込みに失敗しました。{e}"
            ))
        })?;
        lr.repetition = read_u8(reader).map_err(|e| {
            RapReaderError::Unexpected(format!(
                "レベル・反復表の反復数の読み込みに失敗しました。{e}"
            ))
        })?;
    }

    Ok(LevelRepetitionsPart {
        number_of_level_repetitions,
        level_repetitions,
    })
}

/// 観測値を最北西端から経度方向、緯度方向の優先順位で、最南東端まで順に走査して返すイテレーター
///
/// ライフタイム`'a`は、`RapReader`よりも短命なライフタイムを示す。
pub struct RapValueIterator<'a> {
    /// ファイルリーダー
    reader: FileReader,

    /// 圧縮データ全体のバイト数
    compressed_data_bytes: usize,

    /// 経度の最小値（10e-6度単位）
    min_longitude: u32,

    /// 経度方向の格子数
    number_of_h_grids: u16,

    /// 格子の高さ（10e-6度単位）
    grid_height: u32,
    /// 格子の幅（10e-6度単位）
    grid_width: u32,

    /// レベルごとの観測値
    value_by_levels: &'a [u16],
    /// レベル反復数表
    level_repetitions: &'a [LevelRepetition],

    /// 圧縮データを読み込んだバイト数
    read_bytes: usize,
    /// 現在の緯度（10e-6度単位）
    current_latitude: u32,
    /// 現在の経度（10e-6度単位）
    current_longitude: u32,
    /// 経度方向に格子を移動した回数
    h_moved_times: u16,
    /// 現在の観測値
    current_value: Option<u16>,
    /// 現在の観測値を繰り返す回数
    number_of_repetitions: u16,
}

impl<'a> RapValueIterator<'a> {
    /// 観測値を走査して返すイテレーターを構築する。
    ///
    /// 引数`reader`が示すRAPファイル・リーダーの読み込み位置が、圧縮データの先頭位置になっていることを想定している。
    ///
    /// # 引数
    ///
    /// * `reader` - RAPファイル・リーダー
    /// * `compressed_data_bytes` - 圧縮データ全体のバイト数
    /// * `max_latitude` - 観測範囲の最北西端の緯度（10e-6度単位）
    /// * `min_longitude` - 観測範囲の最北西端の経度（10e-6度単位）
    /// * `number_of_h_grids` - 観測範囲の緯度方向の格子数
    /// * `grid_height` - 格子の高さ（10e-6度単位）
    /// * `grid_width` - 格子の幅（10e-6度単位）
    /// * `value_by_levels` - レベルごとの観測値
    /// * `level_repetitions` - レベルと反復数の組み合わせ
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        reader: FileReader,
        compressed_data_bytes: usize,
        max_latitude: u32,
        min_longitude: u32,
        number_of_h_grids: u16,
        grid_height: u32,
        grid_width: u32,
        value_by_levels: &'a [u16],
        level_repetitions: &'a [LevelRepetition],
    ) -> Self {
        Self {
            reader,
            compressed_data_bytes,
            min_longitude,
            number_of_h_grids,
            grid_height,
            grid_width,
            value_by_levels,
            level_repetitions,
            read_bytes: 0,
            current_latitude: max_latitude,
            current_longitude: min_longitude,
            h_moved_times: 0,
            current_value: None,
            number_of_repetitions: 0,
        }
    }

    /// ランレングス圧縮バイトを読み込み。
    fn read_run_length_byte(&mut self) -> RapReaderResult<u8> {
        let mut buf = [0u8; 1];
        self.reader.read_exact(&mut buf).map_err(|e| {
            RapReaderError::Unexpected(format!("データ部の読み込みに失敗しました。{e}"))
        })?;
        self.read_bytes += 1;

        Ok(buf[0])
    }

    /// 圧縮された測定値を読み込む。
    fn expand_run_length(&mut self) -> RapReaderResult<ExpandedValue> {
        // 1バイト読み込み
        let buf = self.read_run_length_byte()?;
        let expanded_value = if buf & 0x80 == 0x00 {
            // レベル・反復表によるランレングス圧縮(a)
            let lr = self.level_repetitions[buf as usize];
            ExpandedValue {
                value: self.value_by_levels[lr.level as usize],
                number_of_repetitions: lr.repetition as u16 + 2,
            }
        } else if buf & 0xE0 == 0xC0 {
            // レベル反復表によらないランレングス圧縮(b)
            let value = self.value_by_levels[(buf & 0x1F) as usize];
            let number_of_repetitions = self.read_run_length_byte()? as u16 + 2;
            ExpandedValue {
                value,
                number_of_repetitions,
            }
        } else if buf & 0xC0 == 0x80 {
            // 頻度が多い単独のレベル値(c)
            let value = self.value_by_levels[(buf & 0x3F) as usize];
            ExpandedValue {
                value,
                number_of_repetitions: 1,
            }
        } else if buf == 0xFE {
            // 頻度が少ない単独のレベル値(d)
            let level = self.read_run_length_byte()? as usize;
            ExpandedValue {
                value: self.value_by_levels[level],
                number_of_repetitions: 1,
            }
        } else {
            return Err(RapReaderError::Unexpected(format!(
                "データ部に判別できないバイトが見つかりました。`0x{buf:x}"
            )));
        };

        Ok(expanded_value)
    }
}

/// 座標と観測値
pub struct LocationValue {
    /// 緯度（度）
    pub latitude: f64,
    /// 経度（度）
    pub longitude: f64,
    /// 観測値
    pub value: Option<u16>,
}

impl<'a> Iterator for RapValueIterator<'a> {
    type Item = RapReaderResult<LocationValue>;

    fn next(&mut self) -> Option<Self::Item> {
        // 現在の観測値の繰り返し回数が0かつ、すべての圧縮データを読み込んだ場合は終了
        if self.number_of_repetitions == 0 && self.compressed_data_bytes <= self.read_bytes {
            return None;
        }

        // 現在の観測値の繰り返し回数が0の場合、圧縮データを読み込み
        if self.number_of_repetitions == 0 {
            let ev = match self.expand_run_length() {
                Ok(ev) => ev,
                Err(e) => return Some(Err(e)),
            };
            self.current_value = if ev.value < u16::MAX {
                Some(ev.value)
            } else {
                None
            };
            self.number_of_repetitions = ev.number_of_repetitions;
        }

        // 結果を生成
        let result = Some(Ok(LocationValue {
            latitude: self.current_latitude as f64 / 1_000_000.0,
            longitude: self.current_longitude as f64 / 1_000_000.0,
            value: self.current_value,
        }));

        // 格子を移動
        self.current_longitude += self.grid_width;
        self.h_moved_times += 1;
        // 経度方向の格子の数だけ緯度方向に移動した場合、現在の格子より1つ南で、最西端の格子に移動
        if self.number_of_h_grids <= self.h_moved_times {
            self.current_latitude -= self.grid_height;
            self.current_longitude = self.min_longitude;
            self.h_moved_times = 0;
        }

        // 現在の観測値を繰り返す回数を減らす
        self.number_of_repetitions -= 1;

        result
    }
}

struct ExpandedValue {
    /// 観測値
    value: u16,
    /// 観測値を返却する回数
    number_of_repetitions: u16,
}
