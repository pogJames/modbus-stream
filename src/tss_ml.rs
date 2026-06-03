//! FFI bindings and safe wrapper for the NXP eIQ Time Series Studio ML model.
//!
//! `libtss_svm.a` is compiled for aarch64 (NXP i.MX93 / cortex-a55).
//! On other architectures `init_model()` is a no-op and `get_model()` returns `None`.
//!
//! Runtime requirement: `algorithm.dat` must be present in the working directory
//! when the server starts (the TSS library reads it during `init()`).

use std::sync::OnceLock;

// ── aarch64 FFI ───────────────────────────────────────────────────────────────

#[cfg(target_arch = "aarch64")]
mod ffi {
    use std::os::raw::c_int;

    pub type TssStatus = c_int;

    /// Algorithm attributes returned by `algo_attribute()`.
    /// Mirrors `tss_algo_attribute_t` from TimeSeries.h exactly.
    ///
    /// C layout on aarch64 (total 56 bytes):
    ///   offset  0: data_tab (u32)             — 0 = interleaved, 1 = channels-first
    ///   offset  4: data_len (u32)             — samples per inference window (1953)
    ///   offset  8: data_dim (u32)             — number of channels (3)
    ///   offset 12: model_size (u32)
    ///   offset 16: model_addr (*f32)
    ///   offset 24: target_num (u32)
    ///   offset 28: recommend_threshold (f32)
    ///   offset 32: lib_id (*c_char)
    ///   offset 40: neutron_verion (*c_char)   — note: typo in NXP header, kept for ABI
    ///   offset 48: recommend_learning_num (u32)
    ///   offset 52: odl_supported (u8)
    ///   offset 53: neutron_enabled (u8)
    ///   offset 54: powerquad_enabled (u8)
    #[repr(C)]
    pub struct TssAlgoAttribute {
        pub data_tab: u32,
        pub data_len: u32,
        pub data_dim: u32,
        pub model_size: u32,
        pub model_addr: *const f32,
        pub target_num: u32,
        pub recommend_threshold: f32,
        pub lib_id: *const std::ffi::c_char,
        pub neutron_verion: *const std::ffi::c_char,
        pub recommend_learning_num: u32,
        pub odl_supported: u8,
        pub neutron_enabled: u8,
        pub powerquad_enabled: u8,
    }

    unsafe impl Send for TssAlgoAttribute {}
    unsafe impl Sync for TssAlgoAttribute {}

    /// Classification task ops.
    #[derive(Clone, Copy)]
    #[repr(C)]
    pub struct TssClsOps {
        pub init: Option<unsafe extern "C" fn() -> TssStatus>,
        pub predict: Option<
            unsafe extern "C" fn(
                data_input: *const f32,
                probabilities: *mut f32,
                class_index: *mut c_int,
            ) -> TssStatus,
        >,
    }

    /// Union over all task-op variants.  Sized for the largest (AD-ODL, 4 fn ptrs = 32 bytes).
    #[repr(C)]
    pub union TssTaskUnion {
        pub cls_ops: TssClsOps,
        pub _max: [u64; 4],
    }

    /// Top-level ops struct returned by `tss_get_task_ops()`.
    ///
    /// C layout on aarch64 (total 48 bytes):
    ///   offset  0: task (i32)
    ///   offset  4: _pad (i32)          — alignment padding before union
    ///   offset  8: ops (union, 32 B)
    ///   offset 40: algo_attribute (fn ptr, 8 B)
    #[repr(C)]
    pub struct TssTaskOps {
        pub task: c_int,
        pub _pad: c_int,
        pub ops: TssTaskUnion,
        pub algo_attribute: Option<unsafe extern "C" fn() -> *const TssAlgoAttribute>,
    }

    unsafe impl Send for TssTaskOps {}
    unsafe impl Sync for TssTaskOps {}

    unsafe extern "C" {
        pub fn tss_get_task_ops() -> *const TssTaskOps;
    }
}

// ── MlModel ───────────────────────────────────────────────────────────────────

/// Safe wrapper around the TSS SVM classification model.
pub struct MlModel {
    #[cfg(target_arch = "aarch64")]
    ops: &'static ffi::TssTaskOps,
    /// 0 = interleaved [x0,y0,z0, x1,y1,z1, …]
    /// 1 = channels-first [x0…xN, y0…yN, z0…zN]
    /// Populated from `algo_attribute().data_tab` at init.
    #[cfg(target_arch = "aarch64")]
    data_tab: u32,
    /// Expected samples per inference (from `algo_attribute().data_len`).
    #[cfg(target_arch = "aarch64")]
    n_samples: usize,
    #[cfg(not(target_arch = "aarch64"))]
    _priv: (),
}

impl MlModel {
    /// Number of samples the model expects per inference window.
    /// Populated from `algo_attribute().data_len` at init; returns 1953 on non-aarch64.
    pub fn data_len(&self) -> usize {
        #[cfg(target_arch = "aarch64")]
        {
            self.n_samples
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            1953
        }
    }
}

unsafe impl Send for MlModel {}
unsafe impl Sync for MlModel {}

impl MlModel {
    fn new() -> anyhow::Result<Self> {
        #[cfg(target_arch = "aarch64")]
        {
            let ptr = unsafe { ffi::tss_get_task_ops() };
            if ptr.is_null() {
                return Err(anyhow::anyhow!("tss_get_task_ops() returned null"));
            }
            // SAFETY: tss_get_task_ops() returns a pointer to static library state.
            let ops: &'static ffi::TssTaskOps = unsafe { &*ptr };

            let init = unsafe { ops.ops.cls_ops.init }
                .ok_or_else(|| anyhow::anyhow!("cls_ops.init is null"))?;
            let status = unsafe { init() };
            if status != 0 {
                return Err(anyhow::anyhow!(
                    "TSS cls_ops.init() returned status {} (check algorithm.dat is in the working directory)",
                    status
                ));
            }

            // Read algorithm attributes to determine correct input layout.
            let (data_tab, data_len, data_dim, target_num) = if let Some(attr_fn) =
                ops.algo_attribute
            {
                let attr_ptr = unsafe { attr_fn() };
                if attr_ptr.is_null() {
                    tracing::warn!(
                        "TSS algo_attribute() returned null — assuming interleaved layout (data_tab=0)"
                    );
                    (0u32, 1953usize, 3usize, 4u32)
                } else {
                    let a = unsafe { &*attr_ptr };
                    (
                        a.data_tab,
                        a.data_len as usize,
                        a.data_dim as usize,
                        a.target_num,
                    )
                }
            } else {
                tracing::warn!(
                    "TSS algo_attribute fn is null — assuming interleaved layout (data_tab=0)"
                );
                (0u32, 1953usize, 3usize, 4u32)
            };

            tracing::info!(
                "TSS ML model ready: data_tab={} ({}), data_len={}, data_dim={}, classes={}",
                data_tab,
                if data_tab == 0 {
                    "interleaved"
                } else {
                    "channels-first"
                },
                data_len,
                data_dim,
                target_num,
            );

            Ok(Self {
                ops,
                data_tab,
                n_samples: data_len,
            })
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            tracing::warn!(
                "TSS ML model not available on this architecture (aarch64 only); \
                 /csv/infer will return errors"
            );
            Ok(Self { _priv: () })
        }
    }

    /// Run classification on `samples` (a slice of (x, y, z) tuples).
    ///
    /// The data is flattened using the layout declared by the model's `algo_attribute`:
    /// - `data_tab = 0` → interleaved: `[x0,y0,z0, x1,y1,z1, …]`
    /// - `data_tab = 1` → channels-first: `[x0,…,xN, y0,…,yN, z0,…,zN]`
    ///
    /// Returns `(class_label, probabilities)` where `class_label` is 1-based (1–4).
    pub fn predict_window(&self, samples: &[(f32, f32, f32)]) -> anyhow::Result<(u32, [f32; 4])> {
        #[cfg(target_arch = "aarch64")]
        {
            if samples.len() != self.n_samples {
                return Err(anyhow::anyhow!(
                    "Model expects {} samples, got {}",
                    self.n_samples,
                    samples.len()
                ));
            }

            let n = samples.len();
            let mut flat: Vec<f32> = Vec::with_capacity(n * 3);

            if self.data_tab == 0 {
                // Interleaved: [x0,y0,z0, x1,y1,z1, …]
                for &(x, y, z) in samples {
                    flat.push(x);
                    flat.push(y);
                    flat.push(z);
                }
            } else {
                // Channels-first: [x0,…,xN, y0,…,yN, z0,…,zN]
                for &(x, _, _) in samples {
                    flat.push(x);
                }
                for &(_, y, _) in samples {
                    flat.push(y);
                }
                for &(_, _, z) in samples {
                    flat.push(z);
                }
            }

            let predict = unsafe { self.ops.ops.cls_ops.predict }
                .ok_or_else(|| anyhow::anyhow!("cls_ops.predict is null"))?;

            let mut probs = [0f32; 4];
            let mut class_idx: std::os::raw::c_int = -1;

            let status = unsafe { predict(flat.as_ptr(), probs.as_mut_ptr(), &mut class_idx) };
            if status != 0 {
                return Err(anyhow::anyhow!(
                    "TSS cls_ops.predict() returned status {}",
                    status
                ));
            }

            // class_idx is 0-based; public labels are 1–4
            Ok(((class_idx as u32) + 1, probs))
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            let _ = samples;
            Err(anyhow::anyhow!("ML inference is only available on aarch64"))
        }
    }
}

// ── Singleton ─────────────────────────────────────────────────────────────────

static MODEL: OnceLock<MlModel> = OnceLock::new();

/// Initialize the global ML model singleton.  Call once at startup.
pub fn init_model() -> anyhow::Result<()> {
    let model = MlModel::new()?;
    let _ = MODEL.set(model);
    Ok(())
}

/// Returns a reference to the initialized model, or `None` if unavailable.
pub fn get_model() -> Option<&'static MlModel> {
    MODEL.get()
}
