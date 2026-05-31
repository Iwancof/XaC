export tick(self):
    if self.output_count("ammo") < 100:
        self.set_recipe("ammo")
    else:
        self.set_recipe("plate")

    if self.can_produce():
        self.produce()
