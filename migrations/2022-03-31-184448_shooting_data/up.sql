create table data (
    team integer not null,
    match_number integer not null,
    alliance text check(alliance in ('red', 'blue')) not null,
    left_tarmac text check (left_tarmac in ('yes', 'no')) not null,
    auto_high_made integer not null,
    auto_high_missed integer not null,
    auto_low_made integer not null,
    auto_low_missed integer not null,
    teleop_high_made integer not null,
    teleop_high_missed integer not null,
    teleop_low_made integer not null,
    teleop_low_missed integer not null,
    climb text check (climb in ('no', 'failed', 'low', 'mid', 'high', 'traversal')) not null,
    primary key (team, match_number)
)
