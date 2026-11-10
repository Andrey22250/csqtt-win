//! Wintun adapter owned by the service.
use std::path::Path;
use std::sync::Arc;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::NetworkManagement::IpHelper::ConvertInterfaceLuidToIndex;

/// Fixed GUID of the adapter: with a random one Windows registers a new network profile
/// ("Network N") on every connect.
const ADAPTER_GUID: u128 = 0x8a1e_4c57_3b2d_4f6a_9c0e_5d71_b3a2_c684;

/// Wintun adapter created by the service. Dropping it deletes the adapter (wintun semantics).
pub struct TunAdapter {
    pub name: String,
    pub luid: u64,
    pub if_index: u32,
    _wintun: wintun::Wintun,
    _adapter: Arc<wintun::Adapter>,
}

impl TunAdapter {
    /// Load wintun.dll from `wintun_dir` (fallback: default search), delete a stale adapter with
    /// the same name if present, create a fresh one (tunnel type "csqtt") and read its LUID and ifIndex.
    pub fn create(name: &str, wintun_dir: &Path) -> anyhow::Result<Self> {
        let wintun = load_wintun(wintun_dir)?;

        // Remove leftover adapter from a previous crash before creating a new one.
        let _ = Self::delete_stale(name, wintun_dir);

        let adapter = wintun::Adapter::create(&wintun, name, "csqtt", Some(ADAPTER_GUID))
            .map_err(|e| anyhow::anyhow!("failed to create wintun adapter '{name}': {e}"))?;

        let luid_raw = adapter.get_luid();
        // SAFETY: Reading the 64-bit integer representation of NET_LUID_LH union.
        let luid = unsafe { luid_raw.Value };

        let mut if_index = 0u32;
        // SAFETY: ConvertInterfaceLuidToIndex takes valid pointers to LUID and destination if_index.
        // wintun uses windows-sys 0.52; rebuild the LUID as our 0.61 type.
        let luid_ours = windows_sys::Win32::NetworkManagement::Ndis::NET_LUID_LH { Value: luid };
        let ret = unsafe { ConvertInterfaceLuidToIndex(&luid_ours, &mut if_index) };
        if ret != ERROR_SUCCESS {
            return Err(anyhow::anyhow!(
                "ConvertInterfaceLuidToIndex failed for adapter '{name}' with error code {ret}"
            ));
        }

        Ok(Self {
            name: name.to_string(),
            luid,
            if_index,
            _wintun: wintun,
            _adapter: adapter,
        })
    }

    /// Remove a leftover adapter with this name (crash cleanup). Ok(false) if none existed.
    ///
    /// In Wintun, an adapter is destroyed when its handles are closed. Opening an existing
    /// adapter by name and dropping the handle closes it and triggers cleanup if no other
    /// process holds it.
    pub fn delete_stale(name: &str, wintun_dir: &Path) -> anyhow::Result<bool> {
        let wintun = load_wintun(wintun_dir)?;
        match wintun::Adapter::open(&wintun, name) {
            Ok(adapter) => {
                // Drop the adapter handle so WintunCloseAdapter is invoked.
                match Arc::try_unwrap(adapter) {
                    Ok(owned) => {
                        let _ = owned.delete();
                    }
                    Err(arc) => {
                        drop(arc);
                    }
                }
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }
}

/// Helper to load wintun.dll from explicit directory with fallback to default search order.
fn load_wintun(wintun_dir: &Path) -> anyhow::Result<wintun::Wintun> {
    let dll_path = wintun_dir.join("wintun.dll");
    // SAFETY: Loading wintun.dll dynamically from wintun_dir.
    let res = unsafe { wintun::load_from_path(&dll_path) };
    match res {
        Ok(w) => Ok(w),
        Err(_) => {
            // SAFETY: Loading wintun.dll using standard Windows dynamic linker search order.
            unsafe { wintun::load() }.map_err(|e| anyhow::anyhow!("failed to load wintun.dll: {e}"))
        }
    }
}


