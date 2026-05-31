export tick(self):
    loop:
        if self.output_blocked():
            return
        self.mine()
        return
