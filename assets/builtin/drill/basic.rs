export tick(self):
    if self.output_blocked():
        return
    self.mine()
