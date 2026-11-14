pub fn status(instances: usize) -> String {
    match instances {
        0 => "Not running".to_owned(),
        1 => "Running".to_owned(),
        n => format!("{n} instances running"),
    }
}

pub fn theme_count(themes: usize) -> String {
    match themes {
        0 => "No themes".to_owned(),
        1 => "1 theme".to_owned(),
        n => format!("{n} themes"),
    }
}
