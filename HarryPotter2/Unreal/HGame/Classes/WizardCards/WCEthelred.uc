//===============================================================================
// #51 Ethelred the Ever-Ready
//===============================================================================

class  WCEthelred extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardEthelredTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Ethelredsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardEthelredBigTexture FILE=TEXTURES\menu\Folio\Cards\Ethelredbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Ethelred the Ever-Ready";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=51
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE150"
    skin(0)=Texture'HProps.skins.WizardCardEthelredTex0'
	textureBig=Texture'WizCardEthelredBigTexture'
	strDescriptionId="WizCard_0099"
}
