//! Minimal CLAP host: loads a plugin bundle, activates it, processes audio offline.
//!
//! Usage: clapcheck <path-to-.so-or-.clap> [plugin-id] [seconds] [sample-rate]

use clap_sys::factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID};
use clap_sys::plugin::clap_plugin_descriptor;
use clap_sys::host::clap_host;
use clap_sys::process::{clap_process, CLAP_PROCESS_CONTINUE};
use clap_sys::version::CLAP_VERSION;
use clap_sys::{audio_buffer::clap_audio_buffer, entry::clap_plugin_entry};
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::PathBuf;

// ---------------------------------------------------------------- host

unsafe extern "C" fn host_get_extension(
    _host: *const clap_host,
    _extension_id: *const c_char,
) -> *const c_void {
    std::ptr::null()
}
unsafe extern "C" fn host_request_restart(_host: *const clap_host) {}
unsafe extern "C" fn host_request_process(_host: *const clap_host) {}
unsafe extern "C" fn host_request_callback(_host: *const clap_host) {}

fn make_host() -> clap_host {
    clap_host {
        clap_version: CLAP_VERSION,
        host_data: std::ptr::null_mut(),
        name: c"serh-clapcheck".as_ptr(),
        vendor: c"sehrnoise".as_ptr(),
        url: c"https://example.invalid".as_ptr(),
        version: c"0.1.0".as_ptr(),
        get_extension: Some(host_get_extension),
        request_restart: Some(host_request_restart),
        request_process: Some(host_request_process),
        request_callback: Some(host_request_callback),
    }
}

// ---------------------------------------------------------------- main

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = PathBuf::from(&args[1]);
    let wanted_id = args.get(2).cloned();
    let seconds: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(2.0);
    let sample_rate: f64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(48000.0);

    // The .clap bundle is a directory containing the shared object.
    let so_path = if path.is_dir() {
        let mut found = None;
        for entry in std::fs::read_dir(&path).unwrap() {
            let entry = entry.unwrap();
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("so") {
                found = Some(p);
                break;
            }
        }
        found.unwrap_or_else(|| panic!("no .so inside {}", path.display()))
    } else {
        path.clone()
    };

    // SAFETY: loading a plugin library; the host owns all entry points it calls.
    unsafe {
        let lib = libloading::Library::new(&so_path).expect("dlopen");

        let entry: *const clap_plugin_entry = {
            let sym: libloading::Symbol<*const clap_plugin_entry> =
                lib.get(b"clap_entry\0").expect("clap_entry symbol");
            *sym
        };
        assert!(!entry.is_null(), "clap_entry is null");
        let entry = &*entry;

        let init = entry.init.expect("clap_entry.init");
        let so_c = CString::new(so_path.to_str().unwrap()).unwrap();
        assert!(init(so_c.as_ptr()), "clap_entry.init failed");

        let get_factory = entry.get_factory.expect("clap_entry.get_factory");
        let factory_ptr = get_factory(CLAP_PLUGIN_FACTORY_ID.as_ptr());
        assert!(!factory_ptr.is_null(), "clap plugin factory not found");
        let factory = &*(factory_ptr as *const clap_plugin_factory);

        // List available plugins.
        let count = (factory.get_plugin_count.unwrap())(factory);
        let mut selected: Option<*const clap_plugin_descriptor> = None;
        for i in 0..count {
            let desc = (factory.get_plugin_descriptor.unwrap())(factory, i);
            let id = CStr::from_ptr((*desc).id).to_string_lossy().to_string();
            let name = CStr::from_ptr((*desc).name).to_string_lossy().to_string();
            println!("plugin[{i}] id={id} name={name}");
            if wanted_id.as_deref() == Some(id.as_str())
                || (wanted_id.is_none() && selected.is_none())
            {
                selected = Some(desc);
            }
        }
        let desc = selected.expect("no plugin found");
        let plugin_id = CStr::from_ptr((*desc).id).to_string_lossy().to_string();
        println!("using plugin id={plugin_id}");

        let host = make_host();
        let plugin_ptr = (factory.create_plugin.unwrap())(factory, &host, (*desc).id);
        assert!(!plugin_ptr.is_null(), "create_plugin failed");
        let plugin = &*plugin_ptr;

        assert!((plugin.init.unwrap())(plugin), "plugin.init failed");

        let block = 512usize;
        let total = (seconds * sample_rate) as usize;
        assert!(
            (plugin.activate.unwrap())(plugin, sample_rate, 1, block as u32),
            "plugin.activate failed"
        );
        assert!(
            (plugin.start_processing.unwrap())(plugin),
            "plugin.start_processing failed"
        );

        // Audio buffers: one stereo in, one stereo out.
        let mut in_l = vec![0f32; block];
        let mut in_r = vec![0f32; block];
        let mut out_l = vec![0f32; block];
        let mut out_r = vec![0f32; block];
        let mut in_ptrs = [in_l.as_mut_ptr(), in_r.as_mut_ptr()];
        let mut out_ptrs = [out_l.as_mut_ptr(), out_r.as_mut_ptr()];
        let input = clap_audio_buffer {
            data32: in_ptrs.as_mut_ptr(),
            data64: std::ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        };
        let mut output = clap_audio_buffer {
            data32: out_ptrs.as_mut_ptr(),
            data64: std::ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        };

        let mut all_l: Vec<f32> = Vec::with_capacity(total);
        let mut all_r: Vec<f32> = Vec::with_capacity(total);
        let mut frames_done = 0usize;
        let mut status = CLAP_PROCESS_CONTINUE;
        while frames_done < total {
            let mut process = clap_process {
                steady_time: frames_done as i64,
                frames_count: block as u32,
                transport: std::ptr::null(),
                audio_inputs: &input,
                audio_outputs: &mut output,
                audio_inputs_count: 1,
                audio_outputs_count: 1,
                in_events: std::ptr::null(),
                out_events: std::ptr::null(),
            };
            status = (plugin.process.unwrap())(plugin, &mut process);
            if status == 0 {
                panic!("plugin.process returned CLAP_PROCESS_ERROR");
            }
            all_l.extend_from_slice(&out_l);
            all_r.extend_from_slice(&out_r);
            frames_done += block;
        }

        let peak = all_l
            .iter()
            .chain(all_r.iter())
            .fold(0f32, |a, &b| a.max(b.abs()));
        let rms = (all_l.iter().map(|v| v * v).sum::<f32>() / all_l.len() as f32).sqrt();
        println!(
            "processed {} frames @ {} Hz, status={}, peak={:.6}, rms={:.6}",
            frames_done, sample_rate, status, peak, rms
        );

        let wav = write_wav("clapcheck-output.wav", &all_l, &all_r, sample_rate as u32);
        println!("wrote {wav}");

        (plugin.stop_processing.unwrap())(plugin);
        (plugin.deactivate.unwrap())(plugin);
        (plugin.destroy.unwrap())(plugin);
    }
}

fn write_wav(path: &str, left: &[f32], right: &[f32], sample_rate: u32) -> String {
    use std::io::Write;
    let n = left.len().min(right.len());
    let data_len = (n * 4) as u32;
    let mut w = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut hdr = Vec::new();
    hdr.extend_from_slice(b"RIFF");
    hdr.extend_from_slice(&(36 + data_len).to_le_bytes());
    hdr.extend_from_slice(b"WAVEfmt ");
    hdr.extend_from_slice(&16u32.to_le_bytes());
    hdr.extend_from_slice(&1u16.to_le_bytes());
    hdr.extend_from_slice(&2u16.to_le_bytes());
    hdr.extend_from_slice(&sample_rate.to_le_bytes());
    hdr.extend_from_slice(&(sample_rate * 4).to_le_bytes());
    hdr.extend_from_slice(&4u16.to_le_bytes());
    hdr.extend_from_slice(&16u16.to_le_bytes());
    hdr.extend_from_slice(b"data");
    hdr.extend_from_slice(&data_len.to_le_bytes());
    w.write_all(&hdr).unwrap();
    for i in 0..n {
        for ch in [left, right] {
            let v = (ch[i].clamp(-1.0, 1.0) * 32767.0) as i16;
            w.write_all(&v.to_le_bytes()).unwrap();
        }
    }
    w.flush().unwrap();
    path.to_string()
}
