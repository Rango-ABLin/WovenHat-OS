//! Bounded process-owned timers and manual-reset events. Object locks are never
//! retained across allocation, completion, scheduler notification or user copy.
use crate::{
    async_op::{self, AsyncClass, Completion, Handle},
    deadline::Timer,
    irq_lock::IrqMutex,
    task::{self, TaskId, TaskPriority},
    timer,
};
const COUNT: usize = 32;
const QUOTA: usize = 8;
const TAG: u64 = 0xc002 << 16;
const MAX_EVENT_TASK_WAITERS: usize = 4;
#[derive(Clone, Copy)]
enum Kind {
    Timer(Timer),
    Event(bool),
}
#[derive(Clone, Copy)]
struct Object {
    generation: u32,
    owner: Option<u64>,
    kind: Kind,
    waiter: Option<Handle>,
    waiter_tasks: [Option<TaskId>; MAX_EVENT_TASK_WAITERS],
}
impl Object {
    const EMPTY: Self = Self {
        generation: 0,
        owner: None,
        kind: Kind::Event(false),
        waiter: None,
        waiter_tasks: [None; MAX_EVENT_TASK_WAITERS],
    };
    fn clear(&mut self) {
        self.owner = None;
        self.waiter = None;
        self.waiter_tasks = [None; MAX_EVENT_TASK_WAITERS];
    }
    fn ready(&mut self, now: u64) -> u64 {
        match &mut self.kind {
            Kind::Timer(timer) => {
                timer.advance(now);
                timer.take()
            }
            Kind::Event(set) => u64::from(*set),
        }
    }
}
static OBJECTS: IrqMutex<[Object; COUNT]> = IrqMutex::with_rank([Object::EMPTY; COUNT], 10);
static WORKER: IrqMutex<Option<TaskId>> = IrqMutex::with_rank(None, 10);

fn lookup(objects: &mut [Object; COUNT], owner: u64, raw: u64) -> Result<&mut Object, ()> {
    if raw & 0xffff_0000 != TAG || raw >> 32 == 0 {
        return Err(());
    }
    let slot = objects.get_mut((raw & 0xffff) as usize).ok_or(())?;
    if slot.owner != Some(owner) || slot.generation != (raw >> 32) as u32 {
        return Err(());
    }
    Ok(slot)
}
fn notify_worker() {
    let worker = *WORKER.lock();
    if let Some(worker) = worker {
        task::signal_event(worker);
    }
}
pub fn start_worker() -> bool {
    if WORKER.lock().is_some() {
        return true;
    }
    match task::spawn_with_priority("async-events", worker, TaskPriority::NORMAL) {
        Ok(id) => {
            *WORKER.lock() = Some(id);
            true
        }
        Err(_) => false,
    }
}
fn create(owner: u64, kind: Kind) -> Result<u64, ()> {
    let raw = {
        let mut objects = OBJECTS.lock();
        if objects.iter().filter(|o| o.owner == Some(owner)).count() >= QUOTA {
            return Err(());
        }
        let (index, object) = objects
            .iter_mut()
            .enumerate()
            .find(|(_, o)| o.owner.is_none() && o.generation < u32::MAX)
            .ok_or(())?;
        object.generation += 1;
        object.owner = Some(owner);
        object.kind = kind;
        object.waiter = None;
        ((object.generation as u64) << 32) | TAG | index as u64
    };
    notify_worker();
    Ok(raw)
}
pub fn create_timer(owner: u64, deadline: u64, period: u64) -> Result<u64, ()> {
    if WORKER.lock().is_none() {
        return Err(());
    }
    create(owner, Kind::Timer(Timer::new(deadline, period).ok_or(())?))
}
pub fn create_event(owner: u64, set: bool) -> Result<u64, ()> {
    create(owner, Kind::Event(set))
}

pub fn wait_current(raw: u64, is_timer: bool) -> Result<Handle, ()> {
    let owner = task::current_process_id();
    let class = if is_timer {
        AsyncClass::Timer
    } else {
        AsyncClass::Event
    };
    let handle = async_op::allocate_current(class).map_err(|_| ())?;
    let result = (|| {
        let mut objects = OBJECTS.lock();
        let object = lookup(&mut objects, owner, raw)?;
        if matches!(object.kind, Kind::Timer(_)) != is_timer
            || object.waiter.is_some()
            || object.waiter_tasks.iter().any(Option::is_some)
        {
            return Err(());
        }
        let count = object.ready(timer::ticks());
        if count != 0 {
            return Ok(Some(count));
        }
        if matches!(object.kind, Kind::Timer(timer) if timer.exhausted()) {
            return Err(());
        }
        object.waiter = Some(handle);
        Ok(None)
    })();
    match result {
        Ok(Some(count)) => {
            let _ = async_op::complete(handle, Completion::new(0, count));
        }
        Ok(None) => notify_worker(),
        Err(()) => {
            let _ = async_op::release_current(handle);
            return Err(());
        }
    }
    Ok(handle)
}

/// Result of registering a task in the unified wait path.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TaskWaitRegistration {
    Ready,
    Registered,
    Busy,
}

/// Register `task` as the level-triggered waiter for one timer/event object.
/// The object lock is released before the caller observes the result. Timer
/// expiry is consumed exactly once; manual-reset events remain ready until
/// cleared by `set_event(..., false)`.
pub fn register_task(
    owner: u64,
    raw: u64,
    is_timer: bool,
    task: TaskId,
) -> Result<TaskWaitRegistration, ()> {
    let result = {
        let mut objects = OBJECTS.lock();
        let object = lookup(&mut objects, owner, raw)?;
        if matches!(object.kind, Kind::Timer(_)) != is_timer {
            return Err(());
        }
        if object.waiter.is_some() {
            return Ok(TaskWaitRegistration::Busy);
        }
        if object.waiter_tasks.contains(&Some(task)) {
            return Ok(TaskWaitRegistration::Registered);
        }
        if object.ready(timer::ticks()) != 0 {
            return Ok(TaskWaitRegistration::Ready);
        }
        if matches!(object.kind, Kind::Timer(timer) if timer.exhausted()) {
            return Err(());
        }
        if is_timer {
            if object.waiter_tasks.iter().any(Option::is_some) {
                return Ok(TaskWaitRegistration::Busy);
            }
            object.waiter_tasks[0] = Some(task);
        } else if let Some(slot) = object.waiter_tasks.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(task);
        } else {
            return Ok(TaskWaitRegistration::Busy);
        }
        TaskWaitRegistration::Registered
    };
    if result == TaskWaitRegistration::Registered {
        notify_worker();
    }
    Ok(result)
}

/// Remove a task registration without touching a different live waiter.
pub fn clear_task_waiter(owner: u64, raw: u64, is_timer: bool, task: TaskId) {
    let mut objects = OBJECTS.lock();
    if let Ok(object) = lookup(&mut objects, owner, raw) {
        if matches!(object.kind, Kind::Timer(_)) == is_timer
            && object.waiter_tasks.contains(&Some(task))
        {
            for waiter in &mut object.waiter_tasks {
                if *waiter == Some(task) {
                    *waiter = None;
                }
            }
        }
    }
}
pub fn set_event(owner: u64, raw: u64, set: bool) -> Result<(), ()> {
    let (waiter, task_waiters) = {
        let mut objects = OBJECTS.lock();
        let object = lookup(&mut objects, owner, raw)?;
        let Kind::Event(state) = &mut object.kind else {
            return Err(());
        };
        *state = set;
        if set {
            (object.waiter.take(), object.waiter_tasks)
        } else {
            (None, [None; MAX_EVENT_TASK_WAITERS])
        }
    };
    if let Some(waiter) = waiter {
        let _ = async_op::complete(waiter, Completion::new(0, 1));
    }
    for task in task_waiters.into_iter().flatten() {
        task::signal_event(task);
    }
    Ok(())
}
pub fn close(owner: u64, raw: u64, is_timer: bool) -> Result<(), ()> {
    let (waiter, task_waiters) = {
        let mut objects = OBJECTS.lock();
        let object = lookup(&mut objects, owner, raw)?;
        if matches!(object.kind, Kind::Timer(_)) != is_timer {
            return Err(());
        }
        let waiter = object.waiter;
        let task_waiters = object.waiter_tasks;
        object.clear();
        (waiter, task_waiters)
    };
    // Current processes have one thread. Stage 11 must extend close to cancel
    // sibling-thread waits without weakening generic operation ownership.
    if let Some(waiter) = waiter {
        let _ = async_op::cancel_current(waiter);
    }
    for task in task_waiters.into_iter().flatten() {
        task::signal_event(task);
    }
    notify_worker();
    Ok(())
}
pub fn cancel(handle: Handle) {
    for object in OBJECTS.lock().iter_mut() {
        if object.waiter == Some(handle) {
            object.waiter = None;
        }
    }
}
/// Called under the scheduler lock, before generic operation owner cleanup.
pub fn release_owner(owner: u64) {
    for object in OBJECTS.lock().iter_mut() {
        if object.owner == Some(owner) {
            object.clear();
        }
    }
}
fn pump(now: u64) -> u64 {
    let mut completions = [None; COUNT];
    let mut deadline = u64::MAX;
    {
        let mut objects = OBJECTS.lock();
        for (index, object) in objects
            .iter_mut()
            .enumerate()
            .filter(|(_, o)| o.owner.is_some())
        {
            if let Kind::Timer(timer) = &mut object.kind {
                timer.advance(now);
                if let Some(next) = timer.next {
                    deadline = deadline.min(next);
                }
            }
            if object.waiter.is_some() {
                let count = object.ready(now);
                if count != 0 {
                    if let Some(waiter) = object.waiter.take() {
                        completions[index] = Some((waiter, count));
                    }
                }
            } else if object.waiter_tasks.iter().any(Option::is_some) {
                let ready = match &object.kind {
                    Kind::Timer(timer) => timer.pending() != 0,
                    Kind::Event(set) => *set,
                };
                if ready {
                    for task in object.waiter_tasks.into_iter().flatten() {
                        task::signal_event(task);
                    }
                }
            }
        }
    }
    for (handle, count) in completions.into_iter().flatten() {
        let _ = async_op::complete(handle, Completion::new(0, count));
    }
    deadline
}
fn worker() -> ! {
    loop {
        let deadline = pump(timer::ticks());
        task::wait_for_event_until(deadline);
    }
}

#[cfg(feature = "stage19-8-test")]
pub fn wait_any_object_self_test() -> bool {
    let owner = task::current_process_id();
    let event = match create_event(owner, true) {
        Ok(raw) => raw,
        Err(()) => return false,
    };
    let ready = matches!(
        register_task(owner, event, false, task::current_task_id()),
        Ok(TaskWaitRegistration::Ready)
    );
    close(owner, event, false).is_ok() && ready
}

#[cfg(feature = "stage19-8-test")]
pub fn event_task_fanout_self_test() -> bool {
    let owner = task::current_process_id();
    let event = match create_event(owner, false) {
        Ok(raw) => raw,
        Err(()) => return false,
    };
    let first = task::TaskId::from_u64(0xfeed_0001);
    let second = task::TaskId::from_u64(0xfeed_0002);
    let registered = matches!(
        register_task(owner, event, false, first),
        Ok(TaskWaitRegistration::Registered)
    ) && matches!(
        register_task(owner, event, false, second),
        Ok(TaskWaitRegistration::Registered)
    );
    let signaled = set_event(owner, event, true).is_ok();
    clear_task_waiter(owner, event, false, first);
    clear_task_waiter(owner, event, false, second);
    close(owner, event, false).is_ok() && registered && signaled
}

#[cfg(feature = "stage10-9-test")]
pub fn active_count() -> usize {
    OBJECTS.lock().iter().filter(|o| o.owner.is_some()).count()
}

#[cfg(feature = "stage10-9-test")]
pub fn structural_self_test() -> bool {
    let owner = task::current_process_id();
    let event = match create_event(owner, false) {
        Ok(raw) => raw,
        Err(()) => return false,
    };
    if set_event(owner, event, true).is_err() {
        return false;
    }
    let event_handle = match wait_current(event, false) {
        Ok(handle) => handle,
        Err(()) => return false,
    };
    let event_result = async_op::wait(event_handle)
        .map(|completion| completion.value == 1)
        .unwrap_or(false);
    if close(owner, event, false).is_err() || !event_result {
        return false;
    }
    let deadline = timer::ticks().saturating_add(2);
    let timer_raw = match create_timer(owner, deadline, 0) {
        Ok(raw) => raw,
        Err(()) => return false,
    };
    let timer_handle = match wait_current(timer_raw, true) {
        Ok(handle) => handle,
        Err(()) => return false,
    };
    let _ = pump(deadline);
    let timer_result = async_op::wait(timer_handle)
        .map(|completion| completion.value == 1)
        .unwrap_or(false);
    close(owner, timer_raw, true).is_ok() && timer_result
}
