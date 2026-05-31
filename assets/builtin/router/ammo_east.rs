export on_item(self, item):
    if item.kind == "ammo":
        if self.push("east", item):
            return
    self.push_any(item)
