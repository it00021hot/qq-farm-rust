use super::super::super::*;

impl FarmDesktop {
    pub(in crate::desktop) fn invalidate_activity_clock(&mut self) {
        self.activities.activity_tick_generation =
            self.activities.activity_tick_generation.wrapping_add(1);
    }

    pub(in crate::desktop) fn start_activity_clock(&mut self, cx: &mut Context<Self>) {
        if self.test_mode {
            return;
        }
        self.invalidate_activity_clock();
        let generation = self.activities.activity_tick_generation;
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let active = this.update(cx, |this, cx| {
                if generation != this.activities.activity_tick_generation
                    || this.page != Page::Activities
                {
                    return false;
                }
                cx.notify();
                true
            });
            if !matches!(active, Ok(true)) {
                break;
            }
        })
        .detach();
    }
}
