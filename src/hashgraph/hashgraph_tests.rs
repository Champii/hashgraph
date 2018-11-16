mod hashgraph_tests {
    use std::collections::HashMap;
    use std::sync::mpsc::{channel, Receiver};
    use std::sync::{Arc, Mutex, RwLock};

    use event::Event;
    use hashgraph::Hashgraph;
    use internal_txs::PeerTx;
    #[allow(unused_imports)]
    use peer::Peer;
    use peers::Peers;
    #[allow(unused_imports)]
    use round::FamousType;

    // new_hash, other_parent
    type EventInsert = (String, String, String, Vec<PeerTx>);

    #[allow(dead_code)]
    fn insert_events(
        hg: &mut Hashgraph,
        indexes: &mut HashMap<String, Event>,
        to_insert: Vec<EventInsert>,
    ) -> bool {
        for event in to_insert.iter() {
            let mut peers = hg
                .get_last_decided_peers()
                .get_peers()
                .values()
                .map(|p| p.clone())
                .collect::<Vec<Peer>>();

            peers.sort_by(|p1, p2| p1.pub_key.cmp(&p2.pub_key));

            let event_hash_bytes = event.0.as_bytes();
            let peers_txs = event.3.clone();

            let peer = event_hash_bytes[0];
            let idx = event_hash_bytes[1];

            let self_hash: &[u8] = &(if idx == 0 { [peer, 0] } else { [peer, idx - 1] });

            let self_p = indexes
                .get(&String::from_utf8(self_hash.to_vec()).unwrap())
                .map_or(0, |ev: &Event| ev.hash);

            let other_p = indexes.get(&event.1).map_or(0, |ev: &Event| ev.hash);

            let tx = event.2.clone().into_bytes();
            let mut txs = vec![];

            if tx.len() > 0 {
                txs = vec![tx];
            }

            let e = Event::new(
                (idx - 48) as u64,
                peers[(peer - 97) as usize].id,
                self_p,
                other_p,
                txs,
                peers_txs,
            );

            // println!("INSERT {}", event.0);

            if !hg.insert_event(e.clone()) {
                return false;
            }

            indexes.insert(event.0.clone(), hg.events.get_event(&e.hash).unwrap());
        }

        true
    }

    #[allow(dead_code)]
    fn insert_events_create(
        to_insert: Vec<EventInsert>,
        peers: Peers,
    ) -> (Hashgraph, HashMap<String, Event>, Receiver<Vec<u8>>) {
        let mut peers = peers.clone();
        let mut indexes = HashMap::new();
        let (tx_out, tx_out_recv) = channel();

        let mut hg = Hashgraph::new(
            // Arc::new(RwLock::new(peers.clone())),
            Arc::new(Mutex::new(tx_out)),
        );

        let mut vpeers = peers
            .clone()
            .get_peers()
            .values()
            .map(|p| p.clone())
            .collect::<Vec<Peer>>();

        vpeers.sort_by(|p1, p2| p1.pub_key.cmp(&p2.pub_key));

        peers.self_id = vpeers[0].id;

        hg.bootstrap(peers.clone());

        for event in to_insert.iter() {
            let event_hash_bytes = event.0.as_bytes();
            let peers_txs = event.3.clone();
            let peer = event_hash_bytes[0];
            let idx = event_hash_bytes[1];

            let self_hash: &[u8] = &(if idx == 0 { [peer, 0] } else { [peer, idx - 1] });

            let self_p = indexes
                .get(&String::from_utf8(self_hash.to_vec()).unwrap())
                .map_or(0, |ev: &Event| ev.hash);

            let other_p = indexes.get(&event.1).map_or(0, |ev: &Event| ev.hash);

            let tx = event.2.clone().into_bytes();
            let mut txs = vec![];

            if tx.len() > 0 {
                txs = vec![tx];
            }

            let e = Event::new(
                (idx - 48) as u64,
                vpeers[(peer - 97) as usize].id,
                self_p,
                other_p,
                txs,
                peers_txs,
            );

            if !hg.insert_event(e.clone()) {
                panic!("Cannot insert event");
            }

            indexes.insert(event.0.clone(), hg.events.get_event(&e.hash).unwrap());
        }

        (hg, indexes, tx_out_recv)
    }

    /*
        b1
       /|
      / |
    a1  |
    | \ |
    |  \|
    a0  b0
    */

    #[test]
    fn simple_test() {
        let mut peers = Peers::new();

        let peer1 = Peer::new("127.0.0.1:1".parse().unwrap(), vec![0]);
        let peer2 = Peer::new("127.0.0.1:2".parse().unwrap(), vec![1]);

        peers.add(peer1.clone());
        peers.add(peer2.clone());

        // (name, other_parent)
        let to_insert = vec![
            ("a0".to_string(), "".to_string(), "".to_string(), vec![]),
            ("b0".to_string(), "".to_string(), "".to_string(), vec![]),
            ("a1".to_string(), "b0".to_string(), "".to_string(), vec![]),
            ("b1".to_string(), "a1".to_string(), "".to_string(), vec![]),
        ];

        let (hg, indexes, _) = insert_events_create(to_insert, peers);

        // ancestor
        let assert_ancestor = |hash1: &str, hash2: &str, res: bool| {
            assert_eq!(
                hg.clone().is_ancestor(
                    indexes.get(hash1).unwrap().clone(),
                    indexes.get(hash2).unwrap().clone()
                ),
                res
            );
        };
        let assert_self_ancestor = |hash1: &str, hash2: &str, res: bool| {
            assert_eq!(
                hg.clone().is_self_ancestor(
                    indexes.get(hash1).unwrap().clone(),
                    indexes.get(hash2).unwrap().clone()
                ),
                res
            );
        };

        let assert_see = |hash1: &str, hash2: &str, res: bool| {
            assert_eq!(
                hg.clone().see(
                    indexes.get(hash1).unwrap().clone(),
                    indexes.get(hash2).unwrap().clone()
                ),
                res
            );
        };

        let assert_strongly_see = |hash1: &str, hash2: &str, res: bool| {
            assert_eq!(
                hg.clone().strongly_see(
                    indexes.get(hash1).unwrap().clone(),
                    indexes.get(hash2).unwrap().clone()
                ),
                res
            );
        };

        let assert_witness = |hash: &str, res: bool| {
            assert_eq!(
                hg.clone().is_witness(indexes.get(hash).unwrap().clone()),
                res
            );
        };

        let assert_round = |hash: &str, res: u64| {
            assert_eq!(
                hg.clone()
                    .events
                    .get_event(&indexes.get(hash).unwrap().clone().hash)
                    .unwrap()
                    .round,
                res
            );
        };

        let assert_first_decendant = |hash1: &str, hash2: &str, res: &str| {
            let ev1 = hg
                .clone()
                .events
                .get_event(&indexes.get(hash1).unwrap().clone().hash)
                .unwrap();

            let ev2 = hg
                .clone()
                .events
                .get_event(&indexes.get(hash2).unwrap().clone().hash)
                .unwrap();

            let ev_res = hg
                .clone()
                .events
                .get_event(&indexes.get(res).unwrap().clone().hash)
                .unwrap();

            assert_eq!(
                hg.clone().get_first_decendant(ev1, ev2).unwrap().hash,
                ev_res.hash
            );
        };

        assert_ancestor("a0", "a1", true);
        assert_ancestor("b0", "a1", true);
        assert_ancestor("a0", "b1", true);
        assert_ancestor("b0", "b1", true);
        assert_ancestor("a1", "b1", true);

        assert_ancestor("b0", "a0", false);
        assert_ancestor("b1", "a0", false);

        // self ancestor

        assert_self_ancestor("a0", "a1", true);
        assert_self_ancestor("b0", "b1", true);

        assert_self_ancestor("a0", "b1", false);
        assert_self_ancestor("b0", "a1", false);

        // see

        assert_see("a1", "a0", true);
        assert_see("a1", "b0", true);
        assert_see("b1", "a0", true);
        assert_see("b1", "b0", true);
        assert_see("b1", "a1", true);

        assert_see("a0", "b0", false);
        assert_see("a0", "b1", false);

        assert_see("b0", "a0", false);
        assert_see("b0", "a1", false);
        assert_see("b0", "b1", false);

        assert_see("a0", "b0", false);
        assert_see("a0", "a1", false);
        assert_see("a0", "b1", false);

        // strongly see

        assert_strongly_see("a1", "b0", true);
        assert_strongly_see("b1", "a0", true);
        assert_strongly_see("b1", "a1", true);
        assert_strongly_see("b1", "b0", true);

        assert_strongly_see("a1", "a0", false);

        // witness

        assert_witness("a0", true);
        assert_witness("b0", true);
        assert_witness("a1", false);
        assert_witness("b1", true);

        // rounds

        assert_round("a0", 1);
        assert_round("b0", 1);
        assert_round("a1", 1);
        assert_round("b1", 2);

        assert_first_decendant("a0", "b1", "a1");
        assert_first_decendant("a1", "b1", "b1");
        assert_first_decendant("b1", "b1", "b1");
        assert_first_decendant("b0", "b1", "b1");
    }

    /*
    |   b4  |
    |   |   |
    |   b3  |
    |  /|   | ---- r1
    | / b2  |
    |/  |   |
    a2  |   |
    | \ |   |
    |   \   |
    |   | \ |
    a1  |   c2
    |   | / |
    |   b1  c1
    | / |   |
    a0  b0  c0
    0   1    2
    */

    #[test]
    fn complex_test() {
        // env_logger::init();

        let mut peers = Peers::new();

        let peer1 = Peer::new("127.0.0.1:1".parse().unwrap(), vec![0]);
        let peer2 = Peer::new("127.0.0.1:2".parse().unwrap(), vec![1]);
        let peer3 = Peer::new("127.0.0.1:3".parse().unwrap(), vec![2]);

        peers.add(peer1.clone());
        peers.add(peer2.clone());
        peers.add(peer3.clone());

        // (name, peer, index, self, other)
        let to_insert = vec![
            ("a0".to_string(), "".to_string(), "".to_string(), vec![]),
            ("b0".to_string(), "".to_string(), "".to_string(), vec![]),
            ("c0".to_string(), "".to_string(), "".to_string(), vec![]),
            ("b1".to_string(), "a0".to_string(), "".to_string(), vec![]),
            ("c1".to_string(), "".to_string(), "".to_string(), vec![]),
            ("c2".to_string(), "b1".to_string(), "".to_string(), vec![]),
            ("a1".to_string(), "".to_string(), "".to_string(), vec![]),
            ("a2".to_string(), "c2".to_string(), "".to_string(), vec![]),
            ("b2".to_string(), "".to_string(), "".to_string(), vec![]),
            ("b3".to_string(), "a2".to_string(), "".to_string(), vec![]),
            ("b4".to_string(), "".to_string(), "".to_string(), vec![]),
        ];

        let (hg, indexes, _) = insert_events_create(to_insert, peers);

        let assert_witness = |hash: &str, res: bool| {
            assert_eq!(
                hg.clone().is_witness(indexes.get(hash).unwrap().clone()),
                res
            );
        };

        let assert_strongly_see = |hash1: &str, hash2: &str, res: bool| {
            assert_eq!(
                hg.clone().strongly_see(
                    indexes.get(hash1).unwrap().clone(),
                    indexes.get(hash2).unwrap().clone()
                ),
                res
            );
        };

        let assert_round = |hash: &str, res: u64| {
            assert_eq!(
                hg.events
                    .get_event(&indexes.get(hash).unwrap().clone().hash)
                    .unwrap()
                    .round,
                res
            );
        };

        let assert_first_decendant = |hash1: &str, hash2: &str, res: &str| {
            let ev1 = hg
                .clone()
                .events
                .get_event(&indexes.get(hash1).unwrap().clone().hash)
                .unwrap();

            let ev2 = hg
                .clone()
                .events
                .get_event(&indexes.get(hash2).unwrap().clone().hash)
                .unwrap();

            let ev_res = hg
                .clone()
                .events
                .get_event(&indexes.get(res).unwrap().clone().hash)
                .unwrap();

            assert_eq!(
                hg.clone().get_first_decendant(ev1, ev2).unwrap().hash,
                ev_res.hash
            );
        };

        // witness

        assert_witness("a0", true);
        assert_witness("b0", true);
        assert_witness("c0", true);
        assert_witness("a1", false);
        assert_witness("b1", false);
        assert_witness("c1", false);
        assert_witness("b3", true);

        // strongly see

        assert_strongly_see("b3", "a0", true);
        assert_strongly_see("b3", "b0", true);
        assert_strongly_see("b3", "c0", true);

        // rounds

        assert_round("a0", 1);
        assert_round("b0", 1);
        assert_round("c0", 1);
        assert_round("a1", 1);
        assert_round("b1", 1);
        assert_round("c1", 1);
        assert_round("b3", 2);
        assert_round("b4", 2);

        assert_first_decendant("a1", "b3", "a2");
    }

    /*
                      Round 5
    		|   |   c9
    		|   | / |
    		|   b9  |
    ------- |  /|   | --------------------------------
    		a9  |   | Round 4
    		| \ |   |
    		|   \   |
    		|   | \ |
    		|   |   c8
    		|   | / |
    		|   b8  |get_mutget_mut
    		| / |   |
    		a8  |   c7
    		| \ | / |
    		|   b7  |
    ------- |  /|   | --------------------------------
    		a7  |   | Round 3
    		| \ |   |
    		|   \   |
    		|   | \ |
    	    |   |   c6
    		|   | / |
    		|   b6  |
    		| / |   |
    		a6  |   c5
    		| \ | / |
    		|   b5  |
    ------- |  /|   | -------------------------------
    		a5  |   |  Round 2           
    		|   |   |                    
    		a4  |   |                    
    		| \ |   |                    
    		|   \   |                    
    		|   | \ |
    	--- a3  |   c4 //a3's other-parent is c1. This situation can happen with concurrency
    	|	|   | / |
    	|	|   b4  |
    	|	| / |   |
    	|	a2  |   c3
    	|	| \ | / |
    	|	|   b3  |
    	|	|   |   |
    	|	|   b2  |
    ----| --|  /|   | ------------------------------
    	|	a1  |   |  Round 1          
    	|	| \ |   |                   
    	|	|   \   |                   
    	|	|   | \ |                   
    	|   |   |   c2                  
    	|	|   |   |
    	----------  c1
    		|   | / |
    		|   b1  |
    	    | / |   |
    		a0  b0  c0
    		0   1    2
    */

    fn create_consensus_hashgraph() -> (Hashgraph, HashMap<String, Event>, Peers) {
        let mut peers = Peers::new();

        let peer1 = Peer::new("127.0.0.1:1".parse().unwrap(), vec![0]);
        let peer2 = Peer::new("127.0.0.1:2".parse().unwrap(), vec![1]);
        let peer3 = Peer::new("127.0.0.1:3".parse().unwrap(), vec![2]);

        peers.add(peer1.clone());
        peers.add(peer2.clone());
        peers.add(peer3.clone());

        // (name, other_parent)
        let to_insert = vec![
            ("a0".to_string(), "".to_string(), "".to_string(), vec![]),
            ("b0".to_string(), "".to_string(), "".to_string(), vec![]),
            ("c0".to_string(), "".to_string(), "".to_string(), vec![]),
            ("b1".to_string(), "a0".to_string(), "".to_string(), vec![]),
            ("c1".to_string(), "b1".to_string(), "c1".to_string(), vec![]),
            ("c2".to_string(), "".to_string(), "".to_string(), vec![]),
            ("a1".to_string(), "c2".to_string(), "".to_string(), vec![]),
            ("b2".to_string(), "a1".to_string(), "".to_string(), vec![]),
            ("b3".to_string(), "".to_string(), "b3".to_string(), vec![]),
            ("c3".to_string(), "b3".to_string(), "".to_string(), vec![]),
            ("a2".to_string(), "b3".to_string(), "".to_string(), vec![]),
            ("b4".to_string(), "a2".to_string(), "".to_string(), vec![]),
            ("c4".to_string(), "b4".to_string(), "".to_string(), vec![]),
            ("a3".to_string(), "c1".to_string(), "".to_string(), vec![]),
            ("a4".to_string(), "c4".to_string(), "".to_string(), vec![]),
            ("a5".to_string(), "".to_string(), "a5".to_string(), vec![]),
            ("b5".to_string(), "a5".to_string(), "".to_string(), vec![]),
            ("a6".to_string(), "b5".to_string(), "b6".to_string(), vec![]),
            ("c5".to_string(), "b5".to_string(), "".to_string(), vec![]),
            ("b6".to_string(), "a6".to_string(), "".to_string(), vec![]),
            ("c6".to_string(), "b6".to_string(), "".to_string(), vec![]),
            ("a7".to_string(), "c6".to_string(), "a7".to_string(), vec![]),
            ("b7".to_string(), "a7".to_string(), "".to_string(), vec![]),
            ("c7".to_string(), "b7".to_string(), "".to_string(), vec![]),
            ("a8".to_string(), "b7".to_string(), "".to_string(), vec![]),
            ("b8".to_string(), "a8".to_string(), "".to_string(), vec![]),
            ("c8".to_string(), "b8".to_string(), "".to_string(), vec![]),
            ("a9".to_string(), "c8".to_string(), "".to_string(), vec![]),
            ("b9".to_string(), "a9".to_string(), "".to_string(), vec![]),
            ("c9".to_string(), "b9".to_string(), "".to_string(), vec![]),
        ];

        let (hg, indexes, _) = insert_events_create(to_insert, peers.clone());

        (hg, indexes, peers)
    }

    #[test]
    fn consensus_test() {
        let (hg, indexes, peers) = create_consensus_hashgraph();

        let assert_round = |hash: &str, res: u64| {
            assert_eq!(
                hg.events
                    .get_event(&indexes.get(hash).unwrap().clone().hash)
                    .unwrap()
                    .round,
                res
            );
        };

        let assert_famous = |r: u64, hash: &str, res: FamousType| {
            let round = hg.rounds.get(&r).unwrap().clone();

            assert_eq!(
                round
                    .events
                    .get(&indexes.get(hash).unwrap().clone().hash)
                    .unwrap()
                    .famous,
                res
            );
        };

        let assert_see = |hash1: &str, hash2: &str, res: bool| {
            assert_eq!(
                hg.clone().see(
                    indexes.get(hash1).unwrap().clone(),
                    indexes.get(hash2).unwrap().clone()
                ),
                res
            );
        };

        let assert_strongly_see = |hash1: &str, hash2: &str, res: bool| {
            assert_eq!(
                hg.clone().strongly_see(
                    indexes.get(hash1).unwrap().clone(),
                    indexes.get(hash2).unwrap().clone()
                ),
                res
            );
        };

        let assert_witness = |hash: &str, res: bool| {
            warn!("WITNESS {}", hash);
            assert_eq!(
                hg.clone().is_witness(indexes.get(hash).unwrap().clone()),
                res
            );
        };

        // Rounds

        assert_witness("a0", true);
        assert_witness("b0", true);
        assert_witness("c0", true);

        assert_strongly_see("a2", "a0", true);
        assert_strongly_see("a2", "b0", true);
        assert_strongly_see("a2", "c0", true);

        assert_strongly_see("b2", "a0", true);
        assert_strongly_see("b2", "b0", true);
        assert_strongly_see("b2", "c0", true);

        assert_strongly_see("c3", "a0", true);
        assert_strongly_see("c3", "b0", true);
        assert_strongly_see("c3", "c0", true);

        assert_strongly_see("b5", "a2", true);
        assert_strongly_see("b5", "b2", true);
        assert_strongly_see("b5", "c3", true);

        assert_witness("a2", true);
        assert_witness("b2", true);
        assert_witness("c3", true);
        assert_witness("b5", true);

        let rounds: Vec<(&str, u64)> = vec![
            ("a0", 1),
            ("b0", 1),
            ("c0", 1),
            ("b1", 1),
            ("c1", 1),
            ("c2", 1),
            ("a1", 1),
            ("b2", 2),
            ("b3", 2),
            ("a2", 2),
            ("c3", 2),
            ("b4", 2),
            ("a3", 2),
            ("c4", 2),
            ("a4", 2),
            ("a5", 2),
            ("b5", 3),
            ("a6", 3),
            ("c5", 3),
            ("b6", 3),
            ("c6", 3),
            ("a7", 3),
            ("b7", 4),
            ("a8", 4),
            ("c7", 4),
            ("b8", 4),
            ("c8", 4),
            ("a9", 4),
            ("b9", 5),
            ("c9", 5),
        ];

        for (hash, round) in rounds.iter() {
            warn!("LOLOLOL {} {}", hash, round);
            assert_round(hash, round.clone());
        }

        // Fame

        assert_see("b2", "a0", true);
        assert_see("b2", "b0", true);
        assert_see("b2", "c0", true);

        assert_see("a2", "a0", true);
        assert_see("a2", "b0", true);
        assert_see("a2", "c0", true);

        assert_see("c3", "a0", true);
        assert_see("c3", "b0", true);
        assert_see("c3", "c0", true);

        assert_see("b5", "b2", true);
        assert_see("b5", "b2", true);
        assert_see("b5", "b2", true);
        assert_see("a6", "b2", true);
        assert_see("a6", "b2", true);
        assert_see("a6", "b2", true);
        assert_see("c5", "b2", true);
        assert_see("c5", "b2", true);
        assert_see("c5", "b2", true);

        assert_see("c0", "b6", false);

        let famous: Vec<(&str, u64, FamousType)> = vec![
            ("a0", 1, FamousType::True),
            ("b0", 1, FamousType::True),
            ("c0", 1, FamousType::True),
            ("b2", 2, FamousType::True),
            ("a2", 2, FamousType::True),
            ("c3", 2, FamousType::True),
            ("b5", 3, FamousType::True),
            ("a6", 3, FamousType::True),
            ("c5", 3, FamousType::True),
        ];

        for (hash, r, val) in famous {
            assert_famous(r, hash, val);
        }

        let undecideds = vec![
            "b5", "b6", "b7", "b8", "b9", "a6", "a7", "a8", "a9", "c5", "c6", "c7", "c8", "c9",
        ];

        //TODO: check that every undecided is in the hg.events.undecided

        assert_eq!(hg.events.undecided.len(), undecideds.len());

        let decided = vec![7, 9, 0, 0, 0];

        for (round, &count) in decided.iter().enumerate() {
            assert_eq!(
                hg.rounds
                    .get(&(round as u64 + 1))
                    .unwrap()
                    .events
                    .iter()
                    .filter(|(_, val)| val.received > 0)
                    .count(),
                count,
            );
        }

        // TODO check the round received

        assert_eq!(hg.transactions[0], "c1".to_string().into_bytes());
        assert_eq!(hg.transactions[1], "b3".to_string().into_bytes());
        assert_eq!(hg.transactions[2], "a5".to_string().into_bytes());
    }

    /*
    We introduce a new participant at Round 2, and remove another participant at
    round 5.
    
    Round 7
    P: [1,2,3]      b9    |
             --------------------
    Round 6          |    |
    P: [1,2,3]       | \  |
                     |   c4
                     | /  |
    				 |    |
                    b8    |
                     |    |
                     |    |
    		 -----------\--------
    Round 5          |   c3
    P: [1,2,3]       | /  |
    				 |    |
                    b7    |
                     |    |
                     |    |
                     | \  |
    		         |   c2
             ----------/---------
    Round 4          |    |
    P:[0,1,2,3]      |    |
                    b6    |
                   / |    |
          		a7   |    |
                |    \    |
                |    |    |
    		    |    |   c1
             ----------/---------
    Round 6     |    |    |
                |   b5    |
                |  / |    |
                a6   |    |
                |    \    |
    		    |    |   c0  <- first peer event
                |    /  
    	       a5    |       <- peer added in this round
            ----|-\--|----
    Round 5     |   b5  
                | /  |  
        	   a4    |       <- peer validated here and added for next round
            ----|-\--|----
    Round 4		|   b4  
    		    | /  |  
    	       a3    |  
            ----|-\--|----
    Round 3     |   b3  
                | /  |  
        	   a2    |  
            ----|-\--|----
    Round 2	    |   b2       <- ask for peer join
                | /  |  
        	   a1    |  
    		 -----\--------
    Round 1	    |   b1  
    		    |  / |  
    		   a0   b0 
    
    			0	 1	  2
    */

    fn create_simple_dyn_hashgraph() -> (Hashgraph, HashMap<String, Event>, Peers, Receiver<Vec<u8>>)
    {
        let mut peers = Peers::new();

        let peer1 = Peer::new("127.0.0.1:1".parse().unwrap(), vec![0]);
        let peer2 = Peer::new("127.0.0.1:2".parse().unwrap(), vec![1]);

        peers.add(peer1.clone());
        peers.add(peer2.clone());

        let peer3 = Peer::new("127.0.0.1:3".parse().unwrap(), vec![2]);

        // (name, other_parent)
        let to_insert = vec![
            ("a0".to_string(), "".to_string(), "".to_string(), vec![]),
            ("b0".to_string(), "".to_string(), "".to_string(), vec![]),
            ("b1".to_string(), "a0".to_string(), "b1".to_string(), vec![]),
            ("a1".to_string(), "b1".to_string(), "a1".to_string(), vec![]),
            (
                "b2".to_string(),
                "a1".to_string(),
                "b2".to_string(),
                vec![PeerTx::new_join(peer3)],
            ),
            ("a2".to_string(), "b2".to_string(), "a2".to_string(), vec![]),
            ("b3".to_string(), "a2".to_string(), "b3".to_string(), vec![]),
            ("a3".to_string(), "b3".to_string(), "a3".to_string(), vec![]),
        ];

        let (hg, indexes, recv) = insert_events_create(to_insert, peers.clone());

        (hg, indexes, peers, recv)
    }

    #[test]
    fn test_simple_dyn_hashgraph() {
        env_logger::init();

        let (mut hg, mut indexes, _peers, _recv) = create_simple_dyn_hashgraph();

        warn!("A3 HASH {:?}", indexes.get("a3").unwrap().hash);

        let rounds: Vec<(&str, u64)> = vec![
            ("a0", 1),
            ("b0", 1),
            ("b1", 1),
            ("a1", 2),
            ("b2", 2),
            ("a2", 3),
            ("b3", 3),
            ("a3", 4),
        ];

        for (hash, round) in rounds.iter() {
            warn!("LOL {}, {}", hash, round);
            assert_eq!(
                hg.events
                    .get_event(&indexes.get(hash.clone()).unwrap().clone().hash)
                    .unwrap()
                    .round,
                round.clone(),
            );
        }

        assert_eq!(hg.rounds.len(), 4);

        assert_eq!(hg.get_last_decided_peers().len(), 2);

        for i in 1..hg.rounds.len() {
            assert_eq!(hg.rounds.get(&(i as u64)).unwrap().peers.len(), 2);
        }

        insert_events(
            &mut hg,
            &mut indexes,
            vec![
                ("b4".to_string(), "a3".to_string(), "b4".to_string(), vec![]),
                ("a4".to_string(), "b4".to_string(), "a4".to_string(), vec![]),
                // ("b5".to_string(), "a4".to_string(), "b5".to_string(), vec![]),
            ],
        );

        error!("LOL {:?}", hg.rounds);

        assert_eq!(hg.rounds.len(), 6);

        // assert_eq!(hg.rounds.len(), 5);

        assert_eq!(hg.get_last_decided_peers().len(), 3);

        let mut hg2 = hg.clone();
        let mut indexes2 = indexes.clone();

        let mut assert_witness = |hash: &str, res: bool| {
            error!("WITNESS {}", hash);
            assert_eq!(hg2.is_witness(indexes2.get(hash).unwrap().clone()), res);
        };

        assert_witness("a0", true);
        assert_witness("b0", true);

        assert_witness("a1", true);
        assert_witness("b2", true);

        assert_witness("a2", true);
        assert_witness("b3", true);

        assert_witness("a3", true);
        assert_witness("b4", true);

        assert_witness("a4", true);

        let rounds: Vec<(&str, u64)> = vec![("b4", 4), ("a4", 5)];

        for (hash, round) in rounds.iter() {
            error!("LOL2 {}, {}", hash, round);
            assert_eq!(
                hg.events
                    .get_event(&indexes.get(hash.clone()).unwrap().clone().hash)
                    .unwrap()
                    .round,
                round.clone(),
            );
        }

        assert_eq!(hg.get_decided_peers(indexes.get("a4").unwrap()).len(), 2);

        // try to add a peer event too soon
        // assert_eq!(
        //     insert_events(
        //         &mut hg,
        //         &mut indexes,
        //         vec![("c0".to_string(), "".to_string(), "c0".to_string(), vec![])],
        //     ),
        //     false
        // );

        assert_eq!(
            insert_events(
                &mut hg,
                &mut indexes,
                vec![
                    ("b5".to_string(), "a4".to_string(), "b5".to_string(), vec![]),
                    ("a5".to_string(), "b5".to_string(), "a5".to_string(), vec![]),
                    ("c0".to_string(), "a5".to_string(), "c0".to_string(), vec![]),
                    // ("b5".to_string(), "a4".to_string(), "b5".to_string(), vec![]),
                ],
            ),
            true
        );

        assert_eq!(hg.rounds.len(), 6);

        let rounds: Vec<(&str, u64)> = vec![("b5", 5), ("a5", 6)];

        for (hash, round) in rounds.iter() {
            error!("LOL3 {}, {}", hash, round);
            assert_eq!(
                hg.events
                    .get_event(&indexes.get(hash.clone()).unwrap().clone().hash)
                    .unwrap()
                    .round,
                round.clone(),
            );
        }

        // assert_eq!(hg.get_last_decided_peers().len(), 3);
    }

    /*
    We introduce a new participant at Round 2, and remove another participant at
    round 5.
    
    Round 7
    P: [1,2,3]      b9    |    |
             ----------\--------------
    Round 6          |   c9    |
    P: [1,2,3]       |    | \  |
                     |    |   d4
                     |    | /  |
    				 |  / |    |
                    b8    |    |
                     | \  |    |
                     |   c8    |
    		 ----------------\--------
    Round 5          |    |   d3
    P: [1,2,3]       |    | /  |
    				 |  / |    |
                    b7    |    |
                     | \  |    |
                     |   c7    |
                     |    | \  |
    		         |    |   d2
             ---------------/---------
    Round 4          |   c6    |
                     | /  |    |
                    b6    |    |
                   / |    |    |
          		a5   |    |    |
                |    \    |    |
                |    |    \    |
    		    |    |    |   d1
             ---------------/---------
    Round 3     |    |   c5    |
                |    | /  |    |
                |   b5    |    |
                |  / |    |    |
                a4   |    |    |
                |    \    |    |
                |    |    \    |
    		    |    |    |   d0
             -------------------------
    Round 2		|    |    | /  |
                |    |   c4    R3
    			|    | /  |
    			|   b4    |
    			| /  |    |
    		   a3    |    |
    		    |  \ |    |
    		    |    | \  |
    		    |    |   c3
    		 -----------/------
    Round 1		|   b3    |
             	| /  |    |
    		   a2    |    |
    		    |  \ |    |
    		    |    | \  |
    		    |    |   c2
    		    |    |  / |
    		    |   b2    | <- ask for peer join
    		 -----/------------
    Round 0	   a1    |    |
                |  \ |    |
    		    |    | \  |
    		    |    |   c1
    		    |    | /  |
    		    |   b1    |
    		    |  / |    |
    		   a0   b0   c0
    
    			0	 1	  2
    */

    fn create_dyn_hashgraph() -> (Hashgraph, HashMap<String, Event>, Peers) {
        let mut peers = Peers::new();

        let peer1 = Peer::new("127.0.0.1:1".parse().unwrap(), vec![0]);
        let peer2 = Peer::new("127.0.0.1:2".parse().unwrap(), vec![1]);
        let peer3 = Peer::new("127.0.0.1:3".parse().unwrap(), vec![2]);

        peers.add(peer1.clone());
        peers.add(peer2.clone());
        peers.add(peer3.clone());

        // (name, other_parent)
        let to_insert = vec![
            ("a0".to_string(), "".to_string(), "".to_string(), vec![]),
            ("b0".to_string(), "".to_string(), "".to_string(), vec![]),
            ("c0".to_string(), "".to_string(), "".to_string(), vec![]),
            ("b1".to_string(), "a0".to_string(), "b1".to_string(), vec![]),
            ("c1".to_string(), "b1".to_string(), "c1".to_string(), vec![]),
            ("a1".to_string(), "c1".to_string(), "a1".to_string(), vec![]),
            ("b2".to_string(), "a1".to_string(), "b2".to_string(), vec![]),
            ("c2".to_string(), "b2".to_string(), "c2".to_string(), vec![]),
            ("a2".to_string(), "c2".to_string(), "a2".to_string(), vec![]),
            ("b3".to_string(), "a2".to_string(), "b3".to_string(), vec![]),
            ("c3".to_string(), "b3".to_string(), "c3".to_string(), vec![]),
            ("a3".to_string(), "c3".to_string(), "a3".to_string(), vec![]),
            ("b4".to_string(), "a3".to_string(), "b4".to_string(), vec![]),
            ("c4".to_string(), "b4".to_string(), "c4".to_string(), vec![]),
        ];

        let (hg, indexes, _) = insert_events_create(to_insert, peers.clone());

        (hg, indexes, peers)
    }

    #[test]
    fn test_dyn_hashgraph() {
        let (mut hg, mut indexes, peers) = create_dyn_hashgraph();

        let peer4 = Peer::new("127.0.0.1:4".parse().unwrap(), vec![3]);
        // add peer
        hg.add_self_event(vec![], vec![PeerTx::new_join(peer4)]);

        let to_insert = vec![
            (
                "w33".to_string(),
                "g21".to_string(),
                "w33".to_string(),
                vec![],
            ),
            (
                "w30".to_string(),
                "w33".to_string(),
                "w30".to_string(),
                vec![],
            ),
            (
                "w31".to_string(),
                "w30".to_string(),
                "w31".to_string(),
                vec![],
            ),
            (
                "w32".to_string(),
                "w31".to_string(),
                "w32".to_string(),
                vec![],
            ),
            (
                "w43".to_string(),
                "w32".to_string(),
                "w43".to_string(),
                vec![],
            ),
            (
                "w40".to_string(),
                "w43".to_string(),
                "w40".to_string(),
                vec![],
            ),
            (
                "w41".to_string(),
                "w40".to_string(),
                "w41".to_string(),
                vec![],
            ),
            (
                "w42".to_string(),
                "w41".to_string(),
                "w42".to_string(),
                vec![],
            ),
        ];

        insert_events(&mut hg, &mut indexes, to_insert);
    }
}
