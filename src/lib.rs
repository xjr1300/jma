use time::PrimitiveDateTime;

pub struct RapReader {
    /// 識別子
    pub identifier: String,

    /// 版番号
    pub version: String,

    /// 作成者コメント
    pub comment: String,

    /// データ数
    ///
    /// データ数が24の場合は、毎正時に観測したデータを記録したファイルを示し、
    /// データ数が48の場合は、30分毎に観測したデータを記録したファイルを示す。
    pub number_of_data: u32,

    /// 観測日時
    pub observation_date_time: PrimitiveDateTime,

    /// 観測要素
    pub observation_element: u16,

    /// 地図種別
    ///
    /// 1: 解析雨量
    pub map_type: u16,

    /// 最初の緯度と軽度
    ///
    /// 0.000001度単位で表現する。
    /// 最初のデータは観測範囲の北西端である。
    /// 最初のデータ以後は、経度方向に西から東にデータが記録され、東端に達したとき、
    /// 格子1つ分だけ南で、西端の格子のデータが記録されている。
    pub start_lat: u32,
    pub start_lon: u32,

    /// 横方向と縦方向の格子間隔
    ///
    /// 0.000001度単位で表現する。
    pub interval_h: u32,
    pub interval_v: u32,

    /// 横方向と縦方向の格子数
    pub number_of_h_grids: u32,
    pub number_of_v_grids: u32,

    /// 圧縮方法
    pub compress_method: u16,

    /// レベル数
    pub number_of_levels: u16,

    /// レベル毎の雨量
    ///
    /// 雨量は0.1mm単位で記録されている。
    /// レベルは`Vec`のインデックスを示す。
    pub preps_by_levels: Vec<u16>,

    /// レベル反復数（繰り返し回数）
    ///
    /// 実際の反復回数は、要素+2回となる。
    /// レベルは`Vec`のインデックスを示す。
    pub number_of_level_repetitions: u16,

    /// 圧縮後のデータ部のサイズ
    pub compressed_data_bytes: u32,

    /// レーダー運用状況
    pub radar_operation_statuses: u64,

    /// 解析に利用したアメダスの総数
    pub number_of_amedas: u32,
}
