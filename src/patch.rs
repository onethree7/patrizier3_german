use windows::Win32::System::{
    Diagnostics::Debug::FlushInstructionCache,
    Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS},
    Threading::GetCurrentProcess,
};

const PAGE_SIZE: usize = 0x1000;

pub(crate) struct Patch {
    pub address: usize,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

struct Page {
    address: usize,
    original: PAGE_PROTECTION_FLAGS,
    writable: bool,
}

// Fault injection exercises protection failures without changing real page permissions.
trait Memory {
    fn protect(
        &mut self,
        address: usize,
        flags: PAGE_PROTECTION_FLAGS,
    ) -> Result<PAGE_PROTECTION_FLAGS, String>;
    unsafe fn write(&mut self, address: usize, bytes: &[u8]);
    fn flush(&mut self) -> Result<(), String>;
}

struct ProcessMemory;

impl Memory for ProcessMemory {
    fn protect(
        &mut self,
        address: usize,
        flags: PAGE_PROTECTION_FLAGS,
    ) -> Result<PAGE_PROTECTION_FLAGS, String> {
        let mut old = PAGE_PROTECTION_FLAGS(0);
        if unsafe { VirtualProtect(address as _, PAGE_SIZE, flags, &mut old) }.as_bool() {
            Ok(old)
        } else {
            Err(format!(
                "VirtualProtect at 0x{address:X}: {}",
                std::io::Error::last_os_error()
            ))
        }
    }

    unsafe fn write(&mut self, address: usize, bytes: &[u8]) {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), address as *mut u8, bytes.len());
    }

    fn flush(&mut self) -> Result<(), String> {
        if unsafe { FlushInstructionCache(GetCurrentProcess(), None, 0) }.as_bool() {
            Ok(())
        } else {
            Err(format!(
                "FlushInstructionCache: {}",
                std::io::Error::last_os_error()
            ))
        }
    }
}

fn page_span(patch: &Patch) -> Result<std::ops::RangeInclusive<usize>, String> {
    if patch.before.is_empty() || patch.before.len() != patch.after.len() {
        return Err("A patch must replace a nonempty, equally sized byte range".into());
    }
    let end = patch
        .address
        .checked_add(patch.before.len() - 1)
        .ok_or("Patch address overflow")?;
    Ok((patch.address & !(PAGE_SIZE - 1))..=(end & !(PAGE_SIZE - 1)))
}

fn restore(memory: &mut impl Memory, pages: &mut [Page]) -> Vec<String> {
    let mut errors = Vec::new();
    for page in pages.iter_mut().rev().filter(|page| page.writable) {
        match memory.protect(page.address, page.original) {
            Ok(_) => page.writable = false,
            Err(error) => errors.push(error),
        }
    }
    errors
}

/// The caller must own the patch targets and keep replacement pointers alive even on error.
/// Install before the game loop starts; this does not suspend other threads.
pub(crate) unsafe fn apply(patches: &[Patch]) -> Result<(), String> {
    apply_with(patches, &mut ProcessMemory)
}

unsafe fn apply_with(patches: &[Patch], memory: &mut impl Memory) -> Result<(), String> {
    let mut addresses = Vec::new();
    for patch in patches {
        addresses.extend(page_span(patch)?.step_by(PAGE_SIZE));
    }
    addresses.sort_unstable();
    addresses.dedup();

    // Acquire every page before changing a single pointer or instruction.
    let mut pages = Vec::with_capacity(addresses.len());
    for address in addresses {
        match memory.protect(address, PAGE_EXECUTE_READWRITE) {
            Ok(original) => pages.push(Page {
                address,
                original,
                writable: true,
            }),
            Err(error) => {
                let restore_errors = restore(memory, &mut pages);
                return Err(format!(
                    "{error}; no bytes changed; protection restore: {restore_errors:?}"
                ));
            }
        }
    }
    for patch in patches {
        memory.write(patch.address, &patch.after);
    }
    let mut errors = memory.flush().err().into_iter().collect::<Vec<_>>();
    errors.extend(restore(memory, &mut pages));
    if errors.is_empty() {
        return Ok(());
    }

    // A restore failure may leave a mixture of writable and protected pages.
    for page in pages.iter_mut().filter(|page| !page.writable) {
        match memory.protect(page.address, PAGE_EXECUTE_READWRITE) {
            Ok(_) => page.writable = true,
            Err(error) => errors.push(format!("Rollback could not acquire page: {error}")),
        }
    }
    let mut skipped = 0;
    for patch in patches.iter().rev() {
        let span = page_span(patch)?;
        if pages
            .iter()
            .filter(|page| span.contains(&page.address))
            .all(|page| page.writable)
        {
            memory.write(patch.address, &patch.before);
        } else {
            skipped += 1;
        }
    }
    errors.extend(memory.flush().err());
    errors.extend(restore(memory, &mut pages));
    Err(format!(
        "Patch transaction failed: {}; rollback skipped {skipped} patch(es)",
        errors.join("; ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use windows::Win32::System::Memory::PAGE_READONLY;

    #[derive(Default)]
    struct FakeMemory {
        calls: usize,
        fail: HashSet<usize>,
        flags: HashMap<usize, PAGE_PROTECTION_FLAGS>,
        writes: Vec<(usize, Vec<u8>)>,
    }

    impl Memory for FakeMemory {
        fn protect(
            &mut self,
            address: usize,
            flags: PAGE_PROTECTION_FLAGS,
        ) -> Result<PAGE_PROTECTION_FLAGS, String> {
            self.calls += 1;
            if self.fail.contains(&self.calls) {
                return Err(format!("injected failure {}", self.calls));
            }
            Ok(self.flags.insert(address, flags).unwrap_or(PAGE_READONLY))
        }
        unsafe fn write(&mut self, address: usize, bytes: &[u8]) {
            let patch = Patch {
                address,
                before: bytes.to_vec(),
                after: bytes.to_vec(),
            };
            for page in page_span(&patch).unwrap().step_by(PAGE_SIZE) {
                assert_eq!(self.flags.get(&page), Some(&PAGE_EXECUTE_READWRITE));
            }
            self.writes.push((address, bytes.to_vec()));
        }
        fn flush(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    fn patches() -> [Patch; 2] {
        [
            Patch {
                address: 0x1ffe,
                before: vec![1; 4],
                after: vec![2; 4],
            },
            Patch {
                address: 0x3000,
                before: vec![3; 4],
                after: vec![4; 4],
            },
        ]
    }

    #[test]
    fn protection_failure_changes_no_bytes() {
        let mut memory = FakeMemory {
            fail: HashSet::from([3]),
            ..Default::default()
        };
        assert!(unsafe { apply_with(&patches(), &mut memory) }.is_err());
        assert!(memory.writes.is_empty());
        assert!(memory.flags.values().all(|flags| *flags == PAGE_READONLY));
    }

    #[test]
    fn restore_failure_rolls_back_bytes_and_protections() {
        let mut memory = FakeMemory {
            fail: HashSet::from([4]),
            ..Default::default()
        };
        assert!(unsafe { apply_with(&patches(), &mut memory) }.is_err());
        assert_eq!(
            memory.writes,
            vec![
                (0x1ffe, vec![2; 4]),
                (0x3000, vec![4; 4]),
                (0x3000, vec![3; 4]),
                (0x1ffe, vec![1; 4])
            ]
        );
        assert!(memory.flags.values().all(|flags| *flags == PAGE_READONLY));
    }

    #[test]
    fn rollback_never_writes_a_page_it_cannot_acquire() {
        let mut memory = FakeMemory {
            fail: HashSet::from([4, 7]),
            ..Default::default()
        };
        let error = unsafe { apply_with(&patches(), &mut memory) }.unwrap_err();
        assert!(error.contains("rollback skipped 1"));
        assert_eq!(memory.writes.len(), 3);
    }

    #[test]
    fn rejects_invalid_patch_spans_without_touching_memory() {
        let mut memory = FakeMemory::default();
        let mut patch = patches().into_iter().next().unwrap();
        patch.address = usize::MAX;
        assert!(unsafe { apply_with(&[patch], &mut memory) }.is_err());
        assert_eq!(memory.calls, 0);
    }
}
