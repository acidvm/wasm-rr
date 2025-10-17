const std = @import("std");

pub fn main() !void {
    const stdout = std.io.getStdOut().writer();
    const stderr = std.io.getStdErr().writer();

    // Get current timestamp
    const timestamp = std.time.milliTimestamp();
    try stderr.print("🎲 FizzBuzz Randomizer started at timestamp: {d}\n", .{timestamp});

    // Initialize random with a seed from crypto random
    var seed: u64 = undefined;
    std.crypto.random.bytes(std.mem.asBytes(&seed));
    var prng = std.Random.DefaultPrng.init(seed);
    const random = prng.random();

    // Generate a random count between 15 and 30
    const count = random.intRangeAtMost(u8, 15, 30);
    try stdout.print("Playing FizzBuzz with {d} numbers:\n", .{count});

    // Play FizzBuzz
    var i: u8 = 1;
    while (i <= count) : (i += 1) {
        const div_by_3 = (i % 3 == 0);
        const div_by_5 = (i % 5 == 0);

        if (div_by_3 and div_by_5) {
            try stdout.print("FizzBuzz\n", .{});
        } else if (div_by_3) {
            try stdout.print("Fizz\n", .{});
        } else if (div_by_5) {
            try stdout.print("Buzz\n", .{});
        } else {
            try stdout.print("{d}\n", .{i});
        }
    }

    // Final timestamp
    const end_timestamp = std.time.milliTimestamp();
    try stderr.print("✨ Finished at timestamp: {d}\n", .{end_timestamp});
}
