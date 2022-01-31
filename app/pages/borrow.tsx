import * as anchor from "@project-serum/anchor";
import {
  Button,
  ButtonGroup,
  Content,
  Dialog,
  DialogContainer,
  Divider,
  Heading as DialogHeading,
  Header,
  Image,
  Flex,
  Form,
  Link as SpectrumLink,
  NumberField,
  View,
  StatusLight,
} from "@adobe/react-spectrum";
import {
  useConnection,
  useWallet,
  useAnchorWallet,
} from "@solana/wallet-adapter-react";
import type { NextPage } from "next";
import { useRouter } from "next/router";
import { useState } from "react";
import { useMutation } from "react-query";
import { Controller, useForm } from "react-hook-form";
import * as api from "../lib/api";
import {
  useNFTByOwnerQuery,
  useMetadataFileQuery,
  NFTResult,
} from "../hooks/query";
import { Card, CardFlexContainer } from "../components/card";
import { ProgressCircle } from "../components/progress";
import { Typography, Heading } from "../components/typography";
import { Main } from "../components/layout";
import { ConnectWalletButton } from "../components/button";

const Borrow: NextPage = () => {
  const { connection } = useConnection();
  const wallet = useWallet();

  const [selected, setDialog] = useState<NFTResult | null>(null);
  const queryResult = useNFTByOwnerQuery(connection, wallet?.publicKey);

  if (!wallet.connected) {
    return (
      <Flex direction="row" justifyContent="center">
        <View marginY="size-2000">
          <ConnectWalletButton />
        </View>
      </Flex>
    );
  }

  return (
    <>
      {queryResult.isLoading ? (
        <ProgressCircle />
      ) : (
        <Main>
          <CardFlexContainer>
            {queryResult.data?.map((nft) => (
              <Card
                key={nft.accountInfo.pubkey?.toBase58()}
                uri={nft.metadata.data?.data?.uri}
              >
                <View paddingX="size-200">
                  <Typography>
                    <Heading size="S">{nft.metadata.data?.data?.name}</Heading>
                  </Typography>
                  <StatusLight
                    marginStart="calc(0px - size-100)"
                    variant="positive"
                  >
                    Verified Collection
                  </StatusLight>
                  <Divider size="S" marginTop="size-600" />
                  <Flex direction="row" justifyContent="right">
                    <Button
                      marginY="size-200"
                      variant="primary"
                      onPress={() => setDialog(nft)}
                    >
                      List
                    </Button>
                  </Flex>
                </View>
              </Card>
            ))}
          </CardFlexContainer>
        </Main>
      )}
      <BorrowDialog nft={selected} setDialog={setDialog} />
    </>
  );
};

interface FormFields {
  amountSOL: number;
  returnAPY: number;
  durationMonths: number;
}

interface BorrowDialogProps {
  nft?: any;
  setDialog: (nft: NFTResult | null) => void;
}

const BorrowDialog: React.FC<BorrowDialogProps> = ({ nft, setDialog }) => {
  const router = useRouter();
  const { connection } = useConnection();
  const anchorWallet = useAnchorWallet();

  const form = useForm<FormFields>();

  const metadataFileQuery = useMetadataFileQuery(nft?.metadata.data?.data?.uri);

  const mutation = useMutation(
    (variables: FormFields) => {
      if (
        anchorWallet &&
        nft?.accountInfo.data.mint &&
        nft?.accountInfo.pubkey
      ) {
        const listingOptions = {
          amount: variables.amountSOL * anchor.web3.LAMPORTS_PER_SOL,
          basisPoints: variables.returnAPY * 10000,
          duration: variables.durationMonths * 30 * 24 * 60 * 60,
        };

        return api.createListing(
          connection,
          anchorWallet,
          nft.accountInfo.data.mint,
          nft.accountInfo.pubkey,
          listingOptions
        );
      }
      throw new Error("Not ready");
    },
    {
      onError(err) {
        console.error("Error: " + err);
      },
      onSuccess() {
        router.push("/borrow");
      },
    }
  );

  return (
    <DialogContainer onDismiss={() => setDialog(null)}>
      {nft && (
        <Dialog>
          <Image
            slot="hero"
            alt="NFT"
            src={metadataFileQuery.data?.image}
            objectFit="cover"
          />
          <DialogHeading>Create Listing</DialogHeading>
          <Header>
            <SpectrumLink>
              <a href="/todo">What&apos;s this?</a>
            </SpectrumLink>
          </Header>
          <Divider />
          <Content>
            <Form
              validationState="invalid"
              onSubmit={form.handleSubmit((data) => mutation.mutate(data))}
            >
              <Controller
                control={form.control}
                name="amountSOL"
                rules={{ required: true }}
                render={({ field: { onChange }, fieldState: { invalid } }) => (
                  <NumberField
                    label="Amount"
                    minValue={0.1}
                    formatOptions={{
                      currency: "SOL",
                    }}
                    validationState={invalid ? "invalid" : undefined}
                    onChange={onChange}
                  />
                )}
              />
              <Controller
                control={form.control}
                name="returnAPY"
                rules={{ required: true }}
                render={({ field: { onChange }, fieldState: { invalid } }) => (
                  <NumberField
                    label="APY"
                    formatOptions={{
                      maximumFractionDigits: 1,
                      style: "percent",
                    }}
                    minValue={0.01}
                    maxValue={6.5}
                    validationState={invalid ? "invalid" : undefined}
                    onChange={onChange}
                  />
                )}
              />
              <Controller
                control={form.control}
                name="durationMonths"
                rules={{ required: true }}
                render={({ field: { onChange }, fieldState: { invalid } }) => (
                  <NumberField
                    label="Duration (months)"
                    minValue={1}
                    maxValue={24}
                    step={1}
                    validationState={invalid ? "invalid" : undefined}
                    onChange={onChange}
                  />
                )}
              />
            </Form>
          </Content>
          <ButtonGroup>
            <Button
              isDisabled={mutation.isLoading}
              variant="secondary"
              onPress={() => setDialog(null)}
            >
              Cancel
            </Button>
            <Button
              isDisabled={mutation.isLoading}
              variant="cta"
              onPress={() => mutation.mutate(form.getValues())}
            >
              Submit
            </Button>
          </ButtonGroup>
        </Dialog>
      )}
    </DialogContainer>
  );
};

export default Borrow;