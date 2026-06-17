use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    sync::{
        Arc, RwLock,
        mpsc::{Receiver, Sender, TryRecvError, channel},
    },
};

struct Letter<W, M>(W, W, M);

type BoxMap<W, M> = Arc<RwLock<HashMap<W, Sender<Letter<W, M>>>>>;

pub struct PostOffice<W, M> {
    box_map: BoxMap<W, M>,
}

impl<W, M> PostOffice<W, M> {
    pub fn new() -> Self {
        Self {
            box_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

pub struct Postbox<W, M>
where
    W: Clone + Copy + PartialEq + Eq + Hash,
{
    owner: W,
    box_map: BoxMap<W, M>,
    rx: Receiver<Letter<W, M>>,
}

impl<W, M> Postbox<W, M>
where
    W: Clone + Copy + PartialEq + Eq + Hash,
{
    pub fn new(po: &mut PostOffice<W, M>, owner: W) -> Self {
        let (tx, rx) = channel();
        if let Ok(map) = po.box_map.write() {
            map
        } else {
            po.box_map.clear_poison();
            po.box_map.write().unwrap()
        }
        .insert(owner, tx);

        Self {
            owner,
            box_map: po.box_map.clone(),
            rx,
        }
    }

    pub fn owner(&self) -> W {
        self.owner
    }

    pub fn list(&self) -> HashSet<W> {
        let mut out = HashSet::new();
        for who in if let Ok(map) = self.box_map.read() {
            map
        } else {
            self.box_map.clear_poison();
            self.box_map.read().unwrap()
        }
        .keys()
        {
            out.insert(*who);
        }
        out
    }

    pub fn try_recv(&self) -> Option<(W, M)> {
        match self.rx.try_recv() {
            Ok(Letter(_, from, msg)) => Some((from, msg)),
            _ => None,
        }
    }

    pub fn try_recv2(&self) -> Result<Option<(W, M)>, String> {
        match self.rx.try_recv() {
            Ok(Letter(_, from, msg)) => Ok(Some((from, msg))),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(TryRecvError::Disconnected.to_string()),
        }
    }

    pub fn recv(&self) -> Option<(W, M)> {
        match self.rx.recv() {
            Ok(Letter(_, from, msg)) => Some((from, msg)),
            Err(_) => None, // Disconnected
        }
    }

    pub fn recv2(&self) -> Result<(W, M), String> {
        match self.rx.recv() {
            Ok(Letter(_, from, msg)) => Ok((from, msg)),
            Err(e) => Err(e.to_string()), // Disconnected
        }
    }

    pub fn send(&mut self, to: W, msg: M) -> bool {
        match if let Ok(rl) = self.box_map.read() {
            rl
        } else {
            self.box_map.clear_poison();
            self.box_map.read().unwrap()
        }
        .get(&to)
        {
            Some(tx) => match tx.send(Letter(to, self.owner, msg)) {
                Ok(_) => true,
                Err(_) => false,
            },
            None => false,
        }
    }

    pub fn send2(&mut self, to: W, msg: M) -> Result<bool, String> {
        match if let Ok(rl) = self.box_map.read() {
            rl
        } else {
            self.box_map.clear_poison();
            self.box_map.read().unwrap()
        }
        .get(&to)
        {
            Some(tx) => match tx.send(Letter(to, self.owner, msg)) {
                Ok(_) => Ok(true),
                Err(e) => Err(e.to_string()),
            },
            None => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum Who {
        Main,
        Slave(i32),
    }

    #[derive(Debug, Clone)]
    pub enum Message {
        Hello,
        ByeBye,
    }

    type PO = PostOffice<Who, Message>;
    type PB = Postbox<Who, Message>;

    #[test]
    fn test() {
        let mut office = PO::new();
        let mut pbox = Postbox::new(&mut office, Who::Main);

        let max_slave = 3;
        for i in 0..max_slave {
            let mut slave_box = PB::new(&mut office, Who::Slave(i));
            thread::spawn(move || {
                println!("[{:?}] wait msg...", slave_box.owner());
                if let Some((who, msg)) = slave_box.recv() {
                    println!("[{:?}] from: {:?}, msg {:?}", slave_box.owner(), who, msg);

                    slave_box.send(who, Message::ByeBye);
                }
            });
        }

        for i in 0..max_slave {
            thread::sleep(Duration::from_secs(1));
            pbox.send(Who::Slave(i), Message::Hello);
        }

        thread::sleep(Duration::from_secs(1));

        let mut i = 0;
        while i < max_slave {
            if let Some((who, msg)) = pbox.try_recv() {
                println!("[{:?}] from: {:?}, msg {:?}", pbox.owner(), who, msg);
                i += 1;
                if i == max_slave {
                    println!("recv all response");
                    break;
                }
            }
        }
    }
}
