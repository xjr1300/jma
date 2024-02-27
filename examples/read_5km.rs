use std::borrow::BorrowMut;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::path::Path;

use time::format_description::FormatItem;
use time::macros::{datetime, format_description};
use time::Duration;

use jma_rap::readers::{output_csv_with_geom, RapReader};

/// ファイル名に付与する日時の書式
const FILE_DATETIME_FMT: &[FormatItem<'_>] =
    format_description!("[year][month][day]T[hour][minute][second]");

fn main() -> anyhow::Result<()> {
    let path = "resources/rap-5km-1991/J1991101.RAP";
    let reader = RapReader::new(path)?;
    let grid_width = reader.grid_width() as f64 / 1e6;
    let grid_height = reader.grid_height() as f64 / 1e6;

    reader.pretty_print(std::io::stdout().borrow_mut())?;

    let mut dt = datetime!(1991-01-01 01:00);
    let end_dt = datetime!(1991-01-02 00:00);
    let dest_dir_path = Path::new("resources/rap-5km-1991");
    while dt <= end_dt {
        let iterator = reader.value_iterator(dt)?;
        let file_name = format!("{}.csv", dt.format(FILE_DATETIME_FMT).unwrap());
        let dest_file_path = dest_dir_path.join(file_name);
        let dest_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(dest_file_path)?;
        let mut writer = BufWriter::new(dest_file);
        output_csv_with_geom(&mut writer, iterator, grid_width, grid_height)?;
        dt += Duration::hours(1);
    }

    Ok(())
}
