export tick(self):
    self.charge_docked_drones()

    if self.stock_count("ammo") > 50:
        self.create_delivery_job({
            item = "ammo",
            amount = 20,
            destination_tag = "frontline"
        })

    self.dispatch_idle_drones()
