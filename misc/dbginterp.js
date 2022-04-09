const { spawn } = require('child_process');
const gdb = spawn('gdb', ['--interpreter=mi', '--command=./dbg_vs.gdb'])
const fs = require('fs')
fs.writeFileSync('dbg.log', ' ');
// fs.open('dbg.log', 'a')

process.stdin.on('data', (data) => {
    fs.appendFileSync('dbg.log', `stdin: ${data}`)
    if (data.includes('solib-search') || data.includes('file')) {
        // process.stdout.write('=cmd-param-changed,param="solib-search-path",value=""\n')
        let n = data.toString().substring(0, 4);
        process.stdout.write("&\"\\n\"\r\n");
        process.stdout.write("^done\r\n");
        process.stdout.write("(gdb)\r\n");
        process.stdout.write(n + '^done\r\n');
        return;
    }
    gdb.stdin.write(data);
})

gdb.stdout.on('data', (data) => {
    if (data.includes('cygdrive')) {
        data = data.toString().replaceAll('/cygdrive/d/', 'D:/');
    }
    if (data.includes('D:\\\\Developement\\\\Projects\\\\RustKernel/C:')) {
        data = data.toString().replaceAll('D:\\\\Developement\\\\Projects\\\\RustKernel/C:', 'C:');
    }
    if (data.includes('D:\\\\Developement\\\\Projects\\\\RustKernel/D:')) {
        data = data.toString().replaceAll('D:\\\\Developement\\\\Projects\\\\RustKernel/D:', 'D:');
    }
    fs.appendFileSync('dbg.log', `stdout: ${data}`)
    process.stdout.write(data)
});

gdb.stderr.on('data', (data) => {
    // if (data.includes('cygdrive')) {
    //     let d = data.toString();
    //     d.replaceAll('/cygdrive/d/', 'D:/');
    //     fs.appendFileSync('dbg.log', `stderr: ${d}`)
    //     process.stderr.write(d)
    //     return;
    // }
    fs.appendFileSync('dbg.log', `stderr: ${data}`)
    process.stderr.write(data)
});

gdb.on('close', (code) => {
    console.log(`child process exited with code ${code}`);
});