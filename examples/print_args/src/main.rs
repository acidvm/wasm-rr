fn main() {
    let mut args = std::env::args();
    if let Some(program) = args.next() {
        println!("program: {}", program);
    }

    for (idx, arg) in args.enumerate() {
        println!("arg{}: {}", idx, arg);
    }
}
