pub fn gear_label(value: u8) -> &'static str {
    match value {
        1 => "1",
        2 => "2",
        3 => "3",
        4 => "4",
        5 => "5",
        8 => "P",
        9 => "N",
        10 => "R1",
        11 => "R2",
        0x0F => "Signal not available",
        _ => "UNKNOWN",
    }
}

pub fn gear_state_label(actual: u8, target: u8) -> String {
    if target == actual {
        gear_label(actual).to_string()
    } else {
        format!("{} -> {}", gear_label(actual), gear_label(target))
    }
}

pub fn profile_label(value: u8) -> &'static str {
    match value {
        0 => "(S)tandard",
        1 => "(C)omfort",
        2 => "(W)inter",
        3 => "(A)gility",
        4 => "(M)anual",
        5 => "(R)ace",
        6 => "(I)ndividual",
        7 => "_Init",
        0xFF => "Unknown",
        _ => "UNKNOWN",
    }
}

pub fn tcc_state_label(value: u8) -> &'static str {
    match value {
        0 => "Open",
        1 => "Slipping",
        2 => "Closed",
        _ => "UNKNOWN",
    }
}

pub fn bool_u8_label(value: u8) -> &'static str {
    match value {
        0 => "No",
        1 => "Yes",
        _ => "UNKNOWN",
    }
}
