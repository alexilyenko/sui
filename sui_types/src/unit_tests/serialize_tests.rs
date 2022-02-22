// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
#![allow(clippy::same_item_push)] // get_key_pair returns random elements

use super::*;
use crate::{
    base_types::*,
    crypto::{get_key_pair, AuthoritySignature},
    object::Object,
};
use std::time::Instant;

// Only relevant in a ser/de context : the `CertifiedOrder` for a transaction is not unique
fn compare_certified_orders(o1: &CertifiedOrder, o2: &CertifiedOrder) {
    assert_eq!(o1.order.digest(), o2.order.digest());
    // in this ser/de context it's relevant to compare signatures
    assert_eq!(o1.signatures, o2.signatures);
}

// Only relevant in a ser/de context : the `CertifiedOrder` for a transaction is not unique
fn compare_object_info_responses(o1: &ObjectInfoResponse, o2: &ObjectInfoResponse) {
    assert_eq!(&o1.object().unwrap(), &o2.object().unwrap());
    assert_eq!(
        o1.object_and_lock.as_ref().unwrap().lock,
        o2.object_and_lock.as_ref().unwrap().lock
    );
    match (
        o1.parent_certificate.as_ref(),
        o2.parent_certificate.as_ref(),
    ) {
        (Some(cert1), Some(cert2)) => {
            assert_eq!(cert1.order.digest(), cert2.order.digest());
            assert_eq!(cert1.signatures, cert2.signatures);
        }
        (None, None) => (),
        _ => panic!("certificate structure between responses differs"),
    }
}

fn random_object_ref() -> ObjectRef {
    (
        ObjectID::random(),
        SequenceNumber::new(),
        ObjectDigest::new([0; 32]),
    )
}

#[test]
fn test_error() {
    let err = SuiError::UnknownSigner;
    let buf = serialize_error(&err);
    let result = deserialize_message(buf.as_slice());
    assert!(result.is_ok());
    if let SerializedMessage::Error(o) = result.unwrap() {
        assert!(*o == err);
    } else {
        panic!()
    }
}

#[test]
fn test_info_request() {
    let req1 = ObjectInfoRequest::latest_object_info_request(dbg_object_id(0x20), None);
    let req2 =
        ObjectInfoRequest::past_object_info_request(dbg_object_id(0x20), SequenceNumber::from(129));

    let buf1 = serialize_object_info_request(&req1);
    let buf2 = serialize_object_info_request(&req2);

    let result1 = deserialize_message(buf1.as_slice());
    let result2 = deserialize_message(buf2.as_slice());
    assert!(result1.is_ok());
    assert!(result2.is_ok());

    if let SerializedMessage::ObjectInfoReq(o) = result1.unwrap() {
        assert_eq!(*o, req1);
    } else {
        panic!()
    }
    if let SerializedMessage::ObjectInfoReq(o) = result2.unwrap() {
        assert_eq!(*o, req2);
    } else {
        panic!()
    }
}

#[test]
fn test_order() {
    let (sender_name, sender_key) = get_key_pair();

    let transfer_order = Order::new_transfer(
        dbg_addr(0x20),
        random_object_ref(),
        sender_name,
        random_object_ref(),
        &sender_key,
    );

    let buf = serialize_order(&transfer_order);
    let result = deserialize_message(buf.as_slice());
    assert!(result.is_ok());
    if let SerializedMessage::Order(o) = result.unwrap() {
        assert!(*o == transfer_order);
    } else {
        panic!()
    }

    let (sender_name, sender_key) = get_key_pair();
    let transfer_order2 = Order::new_transfer(
        dbg_addr(0x20),
        random_object_ref(),
        sender_name,
        random_object_ref(),
        &sender_key,
    );

    let buf = serialize_order(&transfer_order2);
    let result = deserialize_message(buf.as_slice());
    assert!(result.is_ok());
    if let SerializedMessage::Order(o) = result.unwrap() {
        assert!(*o == transfer_order2);
    } else {
        panic!()
    }
}

#[test]
fn test_vote() {
    let (sender_name, sender_key) = get_key_pair();
    let order = Order::new_transfer(
        dbg_addr(0x20),
        random_object_ref(),
        sender_name,
        random_object_ref(),
        &sender_key,
    );

    let (_, authority_key) = get_key_pair();
    let vote = SignedOrder::new(order, *authority_key.public_key_bytes(), &authority_key);

    let buf = serialize_vote(&vote);
    let result = deserialize_message(buf.as_slice());
    assert!(result.is_ok());
    if let SerializedMessage::Vote(o) = result.unwrap() {
        assert!(*o == vote);
    } else {
        panic!()
    }
}

#[test]
fn test_cert() {
    let (sender_name, sender_key) = get_key_pair();
    let order = Order::new_transfer(
        dbg_addr(0x20),
        random_object_ref(),
        sender_name,
        random_object_ref(),
        &sender_key,
    );
    let mut cert = CertifiedOrder {
        order,
        signatures: Vec::new(),
    };

    for _ in 0..3 {
        let (_, authority_key) = get_key_pair();
        let sig = AuthoritySignature::new(&cert.order.data, &authority_key);

        cert.signatures
            .push((*authority_key.public_key_bytes(), sig));
    }

    let buf = serialize_cert(&cert);
    let result = deserialize_message(buf.as_slice());
    assert!(result.is_ok());
    if let SerializedMessage::Cert(o) = result.unwrap() {
        compare_certified_orders(o.as_ref(), &cert);
    } else {
        panic!()
    }
}

#[test]
fn test_info_response() {
    let (sender_name, sender_key) = get_key_pair();
    let order = Order::new_transfer(
        dbg_addr(0x20),
        random_object_ref(),
        sender_name,
        random_object_ref(),
        &sender_key,
    );

    let (_, auth_key) = get_key_pair();
    let vote = SignedOrder::new(order.clone(), *auth_key.public_key_bytes(), &auth_key);

    let mut cert = CertifiedOrder {
        order,
        signatures: Vec::new(),
    };

    for _ in 0..3 {
        let (_, authority_key) = get_key_pair();
        let sig = AuthoritySignature::new(&cert.order.data, &authority_key);

        cert.signatures
            .push((*authority_key.public_key_bytes(), sig));
    }

    let object = Object::with_id_owner_for_testing(dbg_object_id(0x20), dbg_addr(0x20));
    let resp1 = ObjectInfoResponse {
        object_and_lock: Some(ObjectResponse {
            object: object.clone(),
            lock: Some(vote),
            layout: None,
        }),
        parent_certificate: None,
        requested_object_reference: Some(object.to_object_reference()),
    };
    let resp2 = resp1.clone();
    let resp3 = resp1.clone();
    let resp4 = resp1.clone();

    for resp in [resp1, resp2, resp3, resp4].iter() {
        let buf = serialize_object_info_response(resp);
        let result = deserialize_message(buf.as_slice());
        assert!(result.is_ok());
        if let SerializedMessage::ObjectInfoResp(o) = result.unwrap() {
            compare_object_info_responses(o.as_ref(), resp);
        } else {
            panic!()
        }
    }
}

#[test]
fn test_time_order() {
    let (sender_name, sender_key) = get_key_pair();

    let mut buf = Vec::new();
    let now = Instant::now();
    for _ in 0..100 {
        let transfer_order = Order::new_transfer(
            dbg_addr(0x20),
            random_object_ref(),
            sender_name,
            random_object_ref(),
            &sender_key,
        );
        serialize_transfer_order_into(&mut buf, &transfer_order).unwrap();
    }
    println!("Write Order: {} microsec", now.elapsed().as_micros() / 100);

    let mut buf2 = buf.as_slice();
    let now = Instant::now();
    for _ in 0..100 {
        if let SerializedMessage::Order(order) = deserialize_message(&mut buf2).unwrap() {
            order.check_signature().unwrap();
        }
    }
    assert!(deserialize_message(&mut buf2).is_err());
    println!(
        "Read & Check Order: {} microsec",
        now.elapsed().as_micros() / 100
    );
}

#[test]
fn test_time_vote() {
    let (sender_name, sender_key) = get_key_pair();
    let order = Order::new_transfer(
        dbg_addr(0x20),
        random_object_ref(),
        sender_name,
        random_object_ref(),
        &sender_key,
    );

    let (_, authority_key) = get_key_pair();

    let mut buf = Vec::new();
    let now = Instant::now();
    for _ in 0..100 {
        let vote = SignedOrder::new(
            order.clone(),
            *authority_key.public_key_bytes(),
            &authority_key,
        );
        serialize_vote_into(&mut buf, &vote).unwrap();
    }
    println!("Write Vote: {} microsec", now.elapsed().as_micros() / 100);

    let mut buf2 = buf.as_slice();
    let now = Instant::now();
    for _ in 0..100 {
        if let SerializedMessage::Vote(vote) = deserialize_message(&mut buf2).unwrap() {
            vote.signature
                .check(&vote.order.data, vote.authority)
                .unwrap();
        }
    }
    assert!(deserialize_message(&mut buf2).is_err());
    println!(
        "Read & Quickcheck Vote: {} microsec",
        now.elapsed().as_micros() / 100
    );
}

#[test]
fn test_time_cert() {
    let count = 100;
    let (sender_name, sender_key) = get_key_pair();
    let order = Order::new_transfer(
        dbg_addr(0x20),
        random_object_ref(),
        sender_name,
        random_object_ref(),
        &sender_key,
    );
    let mut cert = CertifiedOrder {
        order,
        signatures: Vec::new(),
    };

    use std::collections::HashMap;
    let mut cache = HashMap::new();
    for _ in 0..7 {
        let (_, authority_key) = get_key_pair();
        let sig = AuthoritySignature::new(&cert.order.data, &authority_key);
        cert.signatures
            .push((*authority_key.public_key_bytes(), sig));
        cache.insert(
            *authority_key.public_key_bytes(),
            ed25519_dalek::PublicKey::from_bytes(authority_key.public_key_bytes().as_ref())
                .expect("No problem parsing key."),
        );
    }

    let mut buf = Vec::new();
    let now = Instant::now();

    for _ in 0..count {
        serialize_cert_into(&mut buf, &cert).unwrap();
    }
    println!("Write Cert: {} microsec", now.elapsed().as_micros() / count);

    let now = Instant::now();
    let mut buf2 = buf.as_slice();
    for _ in 0..count {
        if let SerializedMessage::Cert(cert) = deserialize_message(&mut buf2).unwrap() {
            AuthoritySignature::verify_batch(&cert.order.data, &cert.signatures, &cache).unwrap();
        }
    }
    assert!(deserialize_message(buf2).is_err());
    println!(
        "Read & Quickcheck Cert: {} microsec",
        now.elapsed().as_micros() / count
    );
}