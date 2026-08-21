//===============================================================================
// #25 Perpetua Fancourt
//===============================================================================

class  WCFancourt extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardFancourtTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Fancourtsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardFancourtBigTexture FILE=TEXTURES\menu\Folio\Cards\Fancourtbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Perpetua Fancourt";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=25
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE150"
    skin(0)=Texture'HProps.skins.WizardCardFancourtTex0'
	textureBig=Texture'WizCardFancourtBigTexture'
	strDescriptionId="WizCard_0021"
}
